//! # Seer Vertical Patching Engine
//!
//! Vertical patching is a unique self-healing mechanism in Seer. It allows the
//! network to "patch" state errors at specific block heights without requiring a
//! hard fork. This mechanism is critical for maintaining continuity in a highly
//! sharded and complex network.
//!
//! ## Lifecycle
//! 1. A Fisherman node detects an invalid state transition and produces a
//!    `FraudProof` (see `fishermen.rs`).
//! 2. The fraud proof is submitted to the network; a quorum of auditors sign it.
//! 3. A `VerticalPatch` is created referencing the erroneous block height and the
//!    corrected state root.
//! 4. The patch is propagated to all affected shards and applied atomically.

/// The minimum number of auditor signatures required to authorise a patch.
pub const PATCH_QUORUM: usize = 3;

/// A 32-byte hash alias used for block hashes, state roots, and patch IDs.
pub type Hash32 = [u8; 32];

// ─── Patch status ─────────────────────────────────────────────────────────────

/// The lifecycle state of a vertical patch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchStatus {
    /// Patch has been created but not yet signed by a quorum.
    Pending,
    /// Patch has collected enough signatures and is ready to apply.
    Authorised,
    /// Patch has been successfully applied to the local ledger.
    Applied,
    /// Patch was rejected (e.g., invalid proof or insufficient signatures).
    Rejected(String),
}

// ─── Patch types ──────────────────────────────────────────────────────────────

/// Describes the kind of state error a patch corrects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchKind {
    /// An invalid transaction was included in a block.
    InvalidTransaction {
        /// Index of the offending transaction within the block.
        tx_index: u32,
    },
    /// The block's stated state root does not match the computed root.
    StateRootMismatch {
        /// The incorrect root as stored in the block header.
        stored_root: Hash32,
        /// The correct root as computed by auditors.
        correct_root: Hash32,
    },
    /// A double-spend was detected across two conflicting blocks.
    DoubleSpend {
        conflicting_block: Hash32,
    },
}

// ─── Core patch structure ─────────────────────────────────────────────────────

/// A vertical patch: a signed, auditor-authorised correction to a specific
/// block height's state.
#[derive(Debug, Clone)]
pub struct VerticalPatch {
    /// Unique patch identifier (SHA-256 of the patch payload).
    pub id: Hash32,
    /// The block height at which the erroneous state was introduced.
    pub target_height: u64,
    /// Hash of the block being corrected.
    pub target_block_hash: Hash32,
    /// The corrected state root to replace the erroneous one.
    pub corrected_state_root: Hash32,
    /// The kind of error this patch addresses.
    pub kind: PatchKind,
    /// Auditor signatures collected so far (auditor ADNL ID → signature bytes).
    pub signatures: Vec<(Hash32, Vec<u8>)>,
    /// Current lifecycle status.
    pub status: PatchStatus,
}

impl VerticalPatch {
    /// Creates a new pending patch.
    pub fn new(
        id: Hash32,
        target_height: u64,
        target_block_hash: Hash32,
        corrected_state_root: Hash32,
        kind: PatchKind,
    ) -> Self {
        Self {
            id,
            target_height,
            target_block_hash,
            corrected_state_root,
            kind,
            signatures: Vec::new(),
            status: PatchStatus::Pending,
        }
    }

    /// Adds an auditor signature to the patch.
    ///
    /// If the number of valid signatures reaches `PATCH_QUORUM`, the patch
    /// status is automatically advanced to `Authorised`.
    ///
    /// Returns `true` if the signature was newly added (duplicate auditor IDs
    /// are silently ignored).
    pub fn add_signature(&mut self, auditor_id: Hash32, signature: Vec<u8>) -> bool {
        // Reject if already finalised.
        if self.status != PatchStatus::Pending {
            return false;
        }
        // Prevent duplicate signatures from the same auditor.
        if self.signatures.iter().any(|(id, _)| *id == auditor_id) {
            return false;
        }
        self.signatures.push((auditor_id, signature));
        if self.signatures.len() >= PATCH_QUORUM {
            self.status = PatchStatus::Authorised;
        }
        true
    }

    /// Returns `true` if the patch has been authorised by a quorum.
    pub fn is_authorised(&self) -> bool {
        self.status == PatchStatus::Authorised
    }

    /// Marks the patch as applied. Returns an error if it is not yet authorised.
    pub fn mark_applied(&mut self) -> Result<(), PatchError> {
        if self.status != PatchStatus::Authorised {
            return Err(PatchError::NotAuthorised);
        }
        self.status = PatchStatus::Applied;
        Ok(())
    }

    /// Rejects the patch with a reason string.
    pub fn reject(&mut self, reason: impl Into<String>) {
        self.status = PatchStatus::Rejected(reason.into());
    }

    /// Returns the number of signatures collected so far.
    pub fn signature_count(&self) -> usize {
        self.signatures.len()
    }
}

// ─── Patch errors ─────────────────────────────────────────────────────────────

/// Errors that can occur during patch operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchError {
    /// The patch does not yet have a quorum of signatures.
    NotAuthorised,
    /// A patch for this block height already exists.
    DuplicatePatch,
    /// The patch references a block height that is in the future.
    FutureHeight,
    /// The patch's corrected state root is identical to the stored root.
    NoCorrection,
}

impl std::fmt::Display for PatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PatchError::NotAuthorised => write!(f, "patch not yet authorised by quorum"),
            PatchError::DuplicatePatch => write!(f, "a patch for this block height already exists"),
            PatchError::FutureHeight => write!(f, "patch targets a future block height"),
            PatchError::NoCorrection => write!(f, "corrected root is identical to stored root"),
        }
    }
}

// ─── Patch registry ───────────────────────────────────────────────────────────

/// Maintains the set of known vertical patches, keyed by target block height.
#[derive(Debug, Default)]
pub struct PatchRegistry {
    /// height → patch
    patches: std::collections::HashMap<u64, VerticalPatch>,
    /// The current chain tip height (used to reject future-height patches).
    chain_tip: u64,
}

impl PatchRegistry {
    /// Creates a new registry anchored at the given chain tip height.
    pub fn new(chain_tip: u64) -> Self {
        Self {
            patches: std::collections::HashMap::new(),
            chain_tip,
        }
    }

    /// Submits a new patch to the registry.
    ///
    /// Validates that:
    /// - No patch already exists for the target height.
    /// - The target height is not in the future.
    /// - The corrected root differs from the stored root (for `StateRootMismatch`).
    pub fn submit(&mut self, patch: VerticalPatch) -> Result<(), PatchError> {
        if patch.target_height > self.chain_tip {
            return Err(PatchError::FutureHeight);
        }
        if self.patches.contains_key(&patch.target_height) {
            return Err(PatchError::DuplicatePatch);
        }
        if let PatchKind::StateRootMismatch { stored_root, correct_root } = &patch.kind {
            if stored_root == correct_root {
                return Err(PatchError::NoCorrection);
            }
        }
        self.patches.insert(patch.target_height, patch);
        Ok(())
    }

    /// Adds a signature to the patch at the given height.
    pub fn sign(&mut self, height: u64, auditor_id: Hash32, signature: Vec<u8>) -> bool {
        if let Some(patch) = self.patches.get_mut(&height) {
            return patch.add_signature(auditor_id, signature);
        }
        false
    }

    /// Applies the patch at the given height, returning the corrected state root.
    pub fn apply(&mut self, height: u64) -> Result<Hash32, PatchError> {
        let patch = self.patches.get_mut(&height).ok_or(PatchError::NotAuthorised)?;
        patch.mark_applied()?;
        Ok(patch.corrected_state_root)
    }

    /// Returns a reference to the patch at the given height, if any.
    pub fn get(&self, height: u64) -> Option<&VerticalPatch> {
        self.patches.get(&height)
    }

    /// Updates the chain tip (called after each new block is appended).
    pub fn advance_tip(&mut self, new_tip: u64) {
        self.chain_tip = new_tip;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_hash(byte: u8) -> Hash32 {
        [byte; 32]
    }

    fn make_patch(height: u64) -> VerticalPatch {
        VerticalPatch::new(
            dummy_hash(0),
            height,
            dummy_hash(1),
            dummy_hash(2),
            PatchKind::StateRootMismatch {
                stored_root: dummy_hash(1),
                correct_root: dummy_hash(2),
            },
        )
    }

    #[test]
    fn test_patch_reaches_quorum() {
        let mut patch = make_patch(10);
        assert_eq!(patch.status, PatchStatus::Pending);
        for i in 0..PATCH_QUORUM as u8 {
            patch.add_signature(dummy_hash(i + 10), vec![i]);
        }
        assert_eq!(patch.status, PatchStatus::Authorised);
    }

    #[test]
    fn test_duplicate_signature_ignored() {
        let mut patch = make_patch(10);
        let auditor = dummy_hash(99);
        assert!(patch.add_signature(auditor, vec![1]));
        assert!(!patch.add_signature(auditor, vec![2])); // duplicate
        assert_eq!(patch.signature_count(), 1);
    }

    #[test]
    fn test_apply_requires_authorisation() {
        let mut patch = make_patch(10);
        assert_eq!(patch.mark_applied(), Err(PatchError::NotAuthorised));
    }

    #[test]
    fn test_registry_submit_and_apply() {
        let mut registry = PatchRegistry::new(100);
        let patch = make_patch(50);
        registry.submit(patch).unwrap();

        // Sign to quorum.
        for i in 0..PATCH_QUORUM as u8 {
            registry.sign(50, dummy_hash(i + 20), vec![i]);
        }

        let root = registry.apply(50).unwrap();
        assert_eq!(root, dummy_hash(2));
        assert_eq!(registry.get(50).unwrap().status, PatchStatus::Applied);
    }

    #[test]
    fn test_registry_rejects_future_height() {
        let mut registry = PatchRegistry::new(10);
        let patch = make_patch(99);
        assert_eq!(registry.submit(patch), Err(PatchError::FutureHeight));
    }

    #[test]
    fn test_registry_rejects_duplicate() {
        let mut registry = PatchRegistry::new(100);
        registry.submit(make_patch(50)).unwrap();
        assert_eq!(registry.submit(make_patch(50)), Err(PatchError::DuplicatePatch));
    }

    #[test]
    fn test_no_correction_rejected() {
        let mut registry = PatchRegistry::new(100);
        let patch = VerticalPatch::new(
            dummy_hash(0),
            50,
            dummy_hash(1),
            dummy_hash(5),
            PatchKind::StateRootMismatch {
                stored_root: dummy_hash(5),
                correct_root: dummy_hash(5), // same — no correction
            },
        );
        assert_eq!(registry.submit(patch), Err(PatchError::NoCorrection));
    }
}
