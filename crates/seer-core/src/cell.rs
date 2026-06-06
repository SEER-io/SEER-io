//! # Seer Bag-of-Cells (BoC) Implementation
//!
//! Implements the Bag-of-Cells (BoC) data structure — the atomic unit of
//! storage in the Seer ecosystem. All data is represented as a directed
//! acyclic graph (DAG) of cryptographic cells.
//!
//! ## Cell Structure
//! - **Data**: up to 128 bytes of raw payload.
//! - **References**: up to 4 references to child cells (by hash).
//! - **Hash**: SHA-256 over (data ++ sorted child hashes).
//!
//! ## Merkle Proofs
//! The DAG structure enables efficient Merkle proof generation: a light client
//! can verify that a specific cell is part of a root without downloading the
//! entire state.

use std::collections::HashMap;

// ─── Constants ────────────────────────────────────────────────────────────────

/// Maximum bytes of raw data per cell.
pub const MAX_CELL_DATA_BYTES: usize = 128;

/// Maximum number of child references per cell.
pub const MAX_CELL_REFS: usize = 4;

// ─── SHA-256 (inlined — no external deps) ─────────────────────────────────────

fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,
        0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,
        0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,
        0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,
        0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,
        0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,
        0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,
        0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667,0xbb67ae85,0x3c6ef372,0xa54ff53a,
        0x510e527f,0x9b05688c,0x1f83d9ab,0x5be0cd19,
    ];
    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut msg = data.to_vec();
    msg.push(0x80);
    while (msg.len() % 64) != 56 { msg.push(0x00); }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([chunk[i*4],chunk[i*4+1],chunk[i*4+2],chunk[i*4+3]]);
        }
        for i in 16..64 {
            let s0 = w[i-15].rotate_right(7)^w[i-15].rotate_right(18)^(w[i-15]>>3);
            let s1 = w[i-2].rotate_right(17)^w[i-2].rotate_right(19)^(w[i-2]>>10);
            w[i] = w[i-16].wrapping_add(s0).wrapping_add(w[i-7]).wrapping_add(s1);
        }
        let [mut a,mut b,mut c,mut d,mut e,mut f,mut g,mut hh] =
            [h[0],h[1],h[2],h[3],h[4],h[5],h[6],h[7]];
        for i in 0..64 {
            let s1 = e.rotate_right(6)^e.rotate_right(11)^e.rotate_right(25);
            let ch = (e&f)^((!e)&g);
            let t1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2)^a.rotate_right(13)^a.rotate_right(22);
            let maj = (a&b)^(a&c)^(b&c);
            let t2 = s0.wrapping_add(maj);
            hh=g; g=f; f=e; e=d.wrapping_add(t1); d=c; c=b; b=a; a=t1.wrapping_add(t2);
        }
        h[0]=h[0].wrapping_add(a); h[1]=h[1].wrapping_add(b);
        h[2]=h[2].wrapping_add(c); h[3]=h[3].wrapping_add(d);
        h[4]=h[4].wrapping_add(e); h[5]=h[5].wrapping_add(f);
        h[6]=h[6].wrapping_add(g); h[7]=h[7].wrapping_add(hh);
    }
    let mut out = [0u8; 32];
    for (i,&word) in h.iter().enumerate() { out[i*4..(i+1)*4].copy_from_slice(&word.to_be_bytes()); }
    out
}

// ─── Cell ─────────────────────────────────────────────────────────────────────

/// A single BoC cell: up to 128 bytes of data and up to 4 child references.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    /// Raw payload (0–128 bytes).
    pub data: Vec<u8>,
    /// Hashes of child cells (0–4 entries).
    pub refs: Vec<[u8; 32]>,
    /// Cached SHA-256 hash of this cell.
    hash: [u8; 32],
}

impl Cell {
    /// Creates a new cell, computing its hash immediately.
    ///
    /// Returns an error if `data` exceeds `MAX_CELL_DATA_BYTES` or `refs`
    /// exceeds `MAX_CELL_REFS`.
    pub fn new(data: Vec<u8>, refs: Vec<[u8; 32]>) -> Result<Self, CellError> {
        if data.len() > MAX_CELL_DATA_BYTES {
            return Err(CellError::DataTooLarge(data.len()));
        }
        if refs.len() > MAX_CELL_REFS {
            return Err(CellError::TooManyRefs(refs.len()));
        }
        let hash = compute_cell_hash(&data, &refs);
        Ok(Cell { data, refs, hash })
    }

    /// Creates a leaf cell (no children) with the given data.
    pub fn leaf(data: Vec<u8>) -> Result<Self, CellError> {
        Cell::new(data, vec![])
    }

    /// Returns the cell's hash.
    pub fn hash(&self) -> &[u8; 32] {
        &self.hash
    }

    /// Returns `true` if this cell has no children.
    pub fn is_leaf(&self) -> bool {
        self.refs.is_empty()
    }

    /// Returns the number of child references.
    pub fn ref_count(&self) -> usize {
        self.refs.len()
    }
}

/// Computes the canonical hash of a cell: SHA-256(data ++ child_hashes_sorted).
fn compute_cell_hash(data: &[u8], refs: &[[u8; 32]]) -> [u8; 32] {
    let mut input = data.to_vec();
    // Sort refs for canonical ordering (prevents hash collisions from ref reordering).
    let mut sorted_refs = refs.to_vec();
    sorted_refs.sort_unstable();
    for r in &sorted_refs {
        input.extend_from_slice(r);
    }
    sha256(&input)
}

// ─── Cell errors ──────────────────────────────────────────────────────────────

/// Errors from cell operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CellError {
    /// Cell data exceeds the 128-byte limit.
    DataTooLarge(usize),
    /// Cell has more than 4 references.
    TooManyRefs(usize),
    /// A referenced child cell was not found in the bag.
    RefNotFound([u8; 32]),
    /// A circular reference was detected.
    CircularReference,
}

impl std::fmt::Display for CellError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CellError::DataTooLarge(n) => write!(f, "cell data too large: {n} bytes (max {MAX_CELL_DATA_BYTES})"),
            CellError::TooManyRefs(n) => write!(f, "too many refs: {n} (max {MAX_CELL_REFS})"),
            CellError::RefNotFound(h) => write!(f, "referenced cell not found: {:02x}{:02x}...", h[0], h[1]),
            CellError::CircularReference => write!(f, "circular reference detected"),
        }
    }
}

// ─── Bag of Cells ─────────────────────────────────────────────────────────────

/// A collection of cells forming a DAG, with a designated root.
///
/// The bag stores all cells by hash and enforces the DAG invariant (no cycles).
#[derive(Debug, Default)]
pub struct BagOfCells {
    /// All cells in the bag, keyed by their hash.
    cells: HashMap<[u8; 32], Cell>,
    /// The hash of the root cell.
    pub root: Option<[u8; 32]>,
}

impl BagOfCells {
    /// Creates a new, empty bag.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a cell into the bag. Returns the cell's hash.
    ///
    /// Validates that all referenced child hashes are already present in the
    /// bag (children must be inserted before parents).
    pub fn insert(&mut self, cell: Cell) -> Result<[u8; 32], CellError> {
        // Verify all refs exist.
        for r in &cell.refs {
            if !self.cells.contains_key(r) {
                return Err(CellError::RefNotFound(*r));
            }
        }
        let hash = *cell.hash();
        self.cells.insert(hash, cell);
        Ok(hash)
    }

    /// Inserts a cell and sets it as the root of the bag.
    pub fn insert_root(&mut self, cell: Cell) -> Result<[u8; 32], CellError> {
        let hash = self.insert(cell)?;
        self.root = Some(hash);
        Ok(hash)
    }

    /// Retrieves a cell by its hash.
    pub fn get(&self, hash: &[u8; 32]) -> Option<&Cell> {
        self.cells.get(hash)
    }

    /// Returns the root cell, if any.
    pub fn root_cell(&self) -> Option<&Cell> {
        self.root.as_ref().and_then(|h| self.cells.get(h))
    }

    /// Returns the total number of cells in the bag.
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// Returns `true` if the bag contains no cells.
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    // ── Merkle proof ──────────────────────────────────────────────────────

    /// Generates a Merkle proof path from the root to a target cell.
    ///
    /// Returns the sequence of cell hashes from root down to the target,
    /// or `None` if the target is not reachable from the root.
    pub fn merkle_proof(&self, target: &[u8; 32]) -> Option<Vec<[u8; 32]>> {
        let root = self.root?;
        let mut path = Vec::new();
        if self.dfs_path(&root, target, &mut path) {
            Some(path)
        } else {
            None
        }
    }

    fn dfs_path(&self, current: &[u8; 32], target: &[u8; 32], path: &mut Vec<[u8; 32]>) -> bool {
        path.push(*current);
        if current == target {
            return true;
        }
        if let Some(cell) = self.cells.get(current) {
            for child_hash in &cell.refs {
                if self.dfs_path(child_hash, target, path) {
                    return true;
                }
            }
        }
        path.pop();
        false
    }

    /// Verifies a Merkle proof path: checks that each step in the path is
    /// a valid parent-child relationship.
    pub fn verify_proof(&self, proof: &[[u8; 32]]) -> bool {
        if proof.is_empty() {
            return false;
        }
        for window in proof.windows(2) {
            let parent_hash = &window[0];
            let child_hash = &window[1];
            match self.cells.get(parent_hash) {
                Some(parent) => {
                    if !parent.refs.contains(child_hash) {
                        return false;
                    }
                }
                None => return false,
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_leaf_cell_creation() {
        let cell = Cell::leaf(b"hello".to_vec()).unwrap();
        assert!(cell.is_leaf());
        assert_eq!(cell.ref_count(), 0);
        assert_ne!(*cell.hash(), [0u8; 32]);
    }

    #[test]
    fn test_cell_data_too_large() {
        let big = vec![0u8; MAX_CELL_DATA_BYTES + 1];
        assert!(matches!(Cell::leaf(big), Err(CellError::DataTooLarge(_))));
    }

    #[test]
    fn test_cell_too_many_refs() {
        let refs = vec![[0u8; 32]; MAX_CELL_REFS + 1];
        assert!(matches!(Cell::new(vec![], refs), Err(CellError::TooManyRefs(_))));
    }

    #[test]
    fn test_cell_hash_deterministic() {
        let c1 = Cell::leaf(b"data".to_vec()).unwrap();
        let c2 = Cell::leaf(b"data".to_vec()).unwrap();
        assert_eq!(c1.hash(), c2.hash());
    }

    #[test]
    fn test_cell_different_data_different_hash() {
        let c1 = Cell::leaf(b"a".to_vec()).unwrap();
        let c2 = Cell::leaf(b"b".to_vec()).unwrap();
        assert_ne!(c1.hash(), c2.hash());
    }

    #[test]
    fn test_bag_insert_leaf() {
        let mut bag = BagOfCells::new();
        let leaf = Cell::leaf(b"leaf".to_vec()).unwrap();
        let hash = bag.insert(leaf).unwrap();
        assert_eq!(bag.len(), 1);
        assert!(bag.get(&hash).is_some());
    }

    #[test]
    fn test_bag_insert_parent_before_child_fails() {
        let mut bag = BagOfCells::new();
        let fake_child_hash = [0xFFu8; 32];
        let parent = Cell::new(b"parent".to_vec(), vec![fake_child_hash]).unwrap();
        assert!(matches!(bag.insert(parent), Err(CellError::RefNotFound(_))));
    }

    #[test]
    fn test_bag_tree_structure() {
        let mut bag = BagOfCells::new();
        let leaf1 = Cell::leaf(b"leaf1".to_vec()).unwrap();
        let leaf2 = Cell::leaf(b"leaf2".to_vec()).unwrap();
        let h1 = bag.insert(leaf1).unwrap();
        let h2 = bag.insert(leaf2).unwrap();
        let root = Cell::new(b"root".to_vec(), vec![h1, h2]).unwrap();
        let root_hash = bag.insert_root(root).unwrap();
        assert_eq!(bag.len(), 3);
        assert_eq!(bag.root, Some(root_hash));
    }

    #[test]
    fn test_merkle_proof_found() {
        let mut bag = BagOfCells::new();
        let leaf = Cell::leaf(b"target".to_vec()).unwrap();
        let leaf_hash = bag.insert(leaf).unwrap();
        let root = Cell::new(b"root".to_vec(), vec![leaf_hash]).unwrap();
        bag.insert_root(root).unwrap();

        let proof = bag.merkle_proof(&leaf_hash).expect("proof should exist");
        assert_eq!(proof.last(), Some(&leaf_hash));
        assert!(bag.verify_proof(&proof));
    }

    #[test]
    fn test_merkle_proof_not_found() {
        let mut bag = BagOfCells::new();
        let leaf = Cell::leaf(b"leaf".to_vec()).unwrap();
        bag.insert_root(leaf).unwrap();
        assert!(bag.merkle_proof(&[0xABu8; 32]).is_none());
    }

    #[test]
    fn test_cell_hash_ref_order_independent() {
        // Hash should be the same regardless of ref insertion order
        // because we sort refs before hashing.
        let h1 = [1u8; 32];
        let h2 = [2u8; 32];
        let hash_ab = compute_cell_hash(b"data", &[h1, h2]);
        let hash_ba = compute_cell_hash(b"data", &[h2, h1]);
        assert_eq!(hash_ab, hash_ba);
    }
}
