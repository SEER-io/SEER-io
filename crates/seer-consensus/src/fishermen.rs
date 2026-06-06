//! # Seer Fishermen Auditor Logic
//!
//! Fishermen nodes are specialised auditors that monitor the network for
//! fraudulent activity. This module implements stake management, fraud proof
//! verification, and reward allocation for the Fishermen subsystem.
//!
//! ## Lifecycle
//! 1. A node registers as a Fisherman by locking a minimum stake.
//! 2. The Fisherman monitors incoming blocks and transactions for violations.
//! 3. When a violation is detected, a `FraudProof` is constructed and broadcast.
//! 4. If the proof is verified by the network, the Fisherman receives a reward
//!    and the offending validator's stake is slashed.
//! 5. If the proof is invalid, the Fisherman's own stake is slashed.

use std::collections::HashMap;

/// Minimum stake (in base units) required to register as a Fisherman.
pub const MIN_STAKE: u64 = 1_000;

/// Fraction of the offender's slashed stake awarded to the reporting Fisherman
/// (expressed as a numerator over 100, i.e., 30 = 30%).
pub const FISHERMAN_REWARD_PCT: u64 = 30;

/// A 32-byte hash alias.
pub type Hash32 = [u8; 32];

// ─── Fraud proof ──────────────────────────────────────────────────────────────

/// Describes the category of fraud a Fisherman is reporting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FraudKind {
    /// A block contains a transaction that violates the protocol rules.
    InvalidTransaction {
        block_hash: Hash32,
        tx_index: u32,
        /// Serialised transaction bytes for re-verification.
        tx_bytes: Vec<u8>,
    },
    /// A block's stated state root does not match the computed root.
    StateRootMismatch {
        block_hash: Hash32,
        claimed_root: Hash32,
        actual_root: Hash32,
    },
    /// A validator produced two conflicting blocks at the same height.
    EquivocatingBlock {
        block_a: Hash32,
        block_b: Hash32,
        height: u64,
    },
}

/// A cryptographically-bound report of a protocol violation.
#[derive(Debug, Clone)]
pub struct FraudProof {
    /// Unique identifier for this proof (hash of its contents).
    pub id: Hash32,
    /// The ADNL identity of the Fisherman submitting this proof.
    pub reporter: Hash32,
    /// The ADNL identity of the accused validator.
    pub accused: Hash32,
    /// The nature of the alleged fraud.
    pub kind: FraudKind,
    /// Fisherman's signature over the proof payload.
    pub signature: Vec<u8>,
    /// Whether this proof has been verified by the network.
    pub verified: bool,
}

impl FraudProof {
    /// Creates a new, unverified fraud proof.
    pub fn new(
        id: Hash32,
        reporter: Hash32,
        accused: Hash32,
        kind: FraudKind,
        signature: Vec<u8>,
    ) -> Self {
        Self {
            id,
            reporter,
            accused,
            kind,
            signature,
            verified: false,
        }
    }

    /// Performs basic structural validation of the fraud proof.
    ///
    /// In production this would re-execute the offending transaction or
    /// recompute the state root. Here we validate the structural invariants
    /// that can be checked without the full ledger.
    pub fn validate(&self) -> Result<(), FishermanError> {
        // Reporter must not accuse themselves.
        if self.reporter == self.accused {
            return Err(FishermanError::SelfReport);
        }
        // Signature must be non-empty (real ECDSA check would go here).
        if self.signature.is_empty() {
            return Err(FishermanError::InvalidSignature);
        }
        // Kind-specific checks.
        match &self.kind {
            FraudKind::StateRootMismatch { claimed_root, actual_root, .. } => {
                if claimed_root == actual_root {
                    return Err(FishermanError::NoFraudEvidence);
                }
            }
            FraudKind::EquivocatingBlock { block_a, block_b, .. } => {
                if block_a == block_b {
                    return Err(FishermanError::NoFraudEvidence);
                }
            }
            FraudKind::InvalidTransaction { tx_bytes, .. } => {
                if tx_bytes.is_empty() {
                    return Err(FishermanError::NoFraudEvidence);
                }
            }
        }
        Ok(())
    }
}

// ─── Fisherman record ─────────────────────────────────────────────────────────

/// The registration record for a single Fisherman node.
#[derive(Debug, Clone)]
pub struct FishermanRecord {
    /// The node's ADNL identity.
    pub id: Hash32,
    /// Current locked stake in base units.
    pub stake: u64,
    /// Cumulative rewards earned.
    pub rewards_earned: u64,
    /// Number of successful fraud proofs submitted.
    pub successful_reports: u32,
    /// Number of invalid fraud proofs submitted (each incurs a slash).
    pub failed_reports: u32,
    /// Whether this Fisherman is currently active (not slashed out).
    pub active: bool,
}

impl FishermanRecord {
    /// Creates a new Fisherman record with the given initial stake.
    pub fn new(id: Hash32, stake: u64) -> Self {
        Self {
            id,
            stake,
            rewards_earned: 0,
            successful_reports: 0,
            failed_reports: 0,
            active: true,
        }
    }

    /// Adds stake to this Fisherman's locked balance.
    pub fn add_stake(&mut self, amount: u64) {
        self.stake = self.stake.saturating_add(amount);
    }

    /// Slashes a fraction of the Fisherman's stake.
    ///
    /// `slash_pct` is a percentage (0–100). If the remaining stake falls below
    /// `MIN_STAKE`, the Fisherman is deactivated.
    pub fn slash(&mut self, slash_pct: u64) {
        let slash_amount = self.stake.saturating_mul(slash_pct) / 100;
        self.stake = self.stake.saturating_sub(slash_amount);
        if self.stake < MIN_STAKE {
            self.active = false;
        }
    }

    /// Credits a reward to the Fisherman's balance.
    pub fn credit_reward(&mut self, amount: u64) {
        self.rewards_earned = self.rewards_earned.saturating_add(amount);
        self.successful_reports += 1;
    }
}

// ─── Errors ───────────────────────────────────────────────────────────────────

/// Errors from Fisherman operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FishermanError {
    /// The node is not registered as a Fisherman.
    NotRegistered,
    /// The node is already registered.
    AlreadyRegistered,
    /// The provided stake is below the minimum required.
    InsufficientStake,
    /// The Fisherman has been deactivated due to slashing.
    Deactivated,
    /// The fraud proof's signature is invalid or missing.
    InvalidSignature,
    /// The reporter and accused are the same node.
    SelfReport,
    /// The proof does not demonstrate any actual fraud.
    NoFraudEvidence,
    /// A proof with this ID has already been submitted.
    DuplicateProof,
}

impl std::fmt::Display for FishermanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FishermanError::NotRegistered => write!(f, "node is not a registered Fisherman"),
            FishermanError::AlreadyRegistered => write!(f, "node is already registered"),
            FishermanError::InsufficientStake => write!(f, "stake below minimum"),
            FishermanError::Deactivated => write!(f, "Fisherman has been deactivated"),
            FishermanError::InvalidSignature => write!(f, "invalid or missing signature"),
            FishermanError::SelfReport => write!(f, "reporter and accused are the same node"),
            FishermanError::NoFraudEvidence => write!(f, "proof contains no fraud evidence"),
            FishermanError::DuplicateProof => write!(f, "proof ID already submitted"),
        }
    }
}

// ─── Fisherman registry ───────────────────────────────────────────────────────

/// Manages all registered Fisherman nodes and processes fraud proofs.
#[derive(Debug, Default)]
pub struct FishermanRegistry {
    fishermen: HashMap<Hash32, FishermanRecord>,
    /// Submitted proof IDs (to prevent replay).
    seen_proofs: std::collections::HashSet<Hash32>,
}

impl FishermanRegistry {
    /// Creates a new, empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a new Fisherman with the given stake.
    pub fn register(&mut self, id: Hash32, stake: u64) -> Result<(), FishermanError> {
        if self.fishermen.contains_key(&id) {
            return Err(FishermanError::AlreadyRegistered);
        }
        if stake < MIN_STAKE {
            return Err(FishermanError::InsufficientStake);
        }
        self.fishermen.insert(id, FishermanRecord::new(id, stake));
        Ok(())
    }

    /// Adds additional stake to an existing Fisherman.
    pub fn add_stake(&mut self, id: &Hash32, amount: u64) -> Result<(), FishermanError> {
        let record = self.fishermen.get_mut(id).ok_or(FishermanError::NotRegistered)?;
        record.add_stake(amount);
        Ok(())
    }

    /// Submits a fraud proof for processing.
    ///
    /// On success:
    /// - The proof is validated structurally.
    /// - The accused validator's stake is slashed by `FISHERMAN_REWARD_PCT`
    ///   (in a real system, the accused's stake record would live in the ledger).
    /// - The reporting Fisherman receives a reward.
    ///
    /// On failure (invalid proof):
    /// - The reporting Fisherman's stake is slashed.
    pub fn submit_proof(&mut self, proof: FraudProof) -> Result<u64, FishermanError> {
        // Check for duplicate proof.
        if self.seen_proofs.contains(&proof.id) {
            return Err(FishermanError::DuplicateProof);
        }
        // Reporter must be registered and active.
        {
            let reporter = self
                .fishermen
                .get(&proof.reporter)
                .ok_or(FishermanError::NotRegistered)?;
            if !reporter.active {
                return Err(FishermanError::Deactivated);
            }
        }
        self.seen_proofs.insert(proof.id);

        match proof.validate() {
            Ok(()) => {
                // Valid proof: reward the reporter.
                // (Accused slash would be applied to the ledger in production.)
                let reward = MIN_STAKE * FISHERMAN_REWARD_PCT / 100;
                if let Some(reporter) = self.fishermen.get_mut(&proof.reporter) {
                    reporter.credit_reward(reward);
                }
                Ok(reward)
            }
            Err(e) => {
                // Invalid proof: slash the reporter.
                if let Some(reporter) = self.fishermen.get_mut(&proof.reporter) {
                    reporter.slash(10); // 10% slash for false report
                    reporter.failed_reports += 1;
                }
                Err(e)
            }
        }
    }

    /// Returns a reference to a Fisherman record.
    pub fn get(&self, id: &Hash32) -> Option<&FishermanRecord> {
        self.fishermen.get(id)
    }

    /// Returns the number of registered (including deactivated) Fishermen.
    pub fn count(&self) -> usize {
        self.fishermen.len()
    }

    /// Returns the number of currently active Fishermen.
    pub fn active_count(&self) -> usize {
        self.fishermen.values().filter(|f| f.active).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> Hash32 {
        [byte; 32]
    }

    fn valid_proof(reporter: Hash32, accused: Hash32) -> FraudProof {
        FraudProof::new(
            id(0xAA),
            reporter,
            accused,
            FraudKind::StateRootMismatch {
                block_hash: id(1),
                claimed_root: id(2),
                actual_root: id(3),
            },
            vec![0xDE, 0xAD, 0xBE, 0xEF],
        )
    }

    #[test]
    fn test_register_and_stake() {
        let mut reg = FishermanRegistry::new();
        reg.register(id(1), MIN_STAKE).unwrap();
        assert_eq!(reg.count(), 1);
        reg.add_stake(&id(1), 500).unwrap();
        assert_eq!(reg.get(&id(1)).unwrap().stake, MIN_STAKE + 500);
    }

    #[test]
    fn test_register_below_min_stake() {
        let mut reg = FishermanRegistry::new();
        assert_eq!(reg.register(id(1), 100), Err(FishermanError::InsufficientStake));
    }

    #[test]
    fn test_duplicate_registration() {
        let mut reg = FishermanRegistry::new();
        reg.register(id(1), MIN_STAKE).unwrap();
        assert_eq!(reg.register(id(1), MIN_STAKE), Err(FishermanError::AlreadyRegistered));
    }

    #[test]
    fn test_valid_proof_rewards_reporter() {
        let mut reg = FishermanRegistry::new();
        reg.register(id(1), MIN_STAKE).unwrap();
        let proof = valid_proof(id(1), id(2));
        let reward = reg.submit_proof(proof).unwrap();
        assert!(reward > 0);
        assert_eq!(reg.get(&id(1)).unwrap().successful_reports, 1);
    }

    #[test]
    fn test_duplicate_proof_rejected() {
        let mut reg = FishermanRegistry::new();
        reg.register(id(1), MIN_STAKE).unwrap();
        let proof = valid_proof(id(1), id(2));
        reg.submit_proof(proof.clone()).unwrap();
        assert_eq!(reg.submit_proof(proof), Err(FishermanError::DuplicateProof));
    }

    #[test]
    fn test_self_report_rejected() {
        let mut reg = FishermanRegistry::new();
        reg.register(id(1), MIN_STAKE).unwrap();
        let proof = valid_proof(id(1), id(1)); // reporter == accused
        let result = reg.submit_proof(proof);
        assert_eq!(result, Err(FishermanError::SelfReport));
    }

    #[test]
    fn test_slash_deactivates_below_min() {
        let mut record = FishermanRecord::new(id(1), MIN_STAKE);
        record.slash(95); // slash 95% → stake < MIN_STAKE
        assert!(!record.active);
    }

    #[test]
    fn test_active_count() {
        let mut reg = FishermanRegistry::new();
        reg.register(id(1), MIN_STAKE).unwrap();
        reg.register(id(2), MIN_STAKE).unwrap();
        assert_eq!(reg.active_count(), 2);
    }
}
