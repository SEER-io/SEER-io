//! # Seer Block Structure
//!
//! Defines the block format used in the Seer blockchain. A block is the
//! fundamental unit of consensus: it bundles transactions, links to the
//! previous block, and carries the PoW solution that proves work was done.
//!
//! ## Block Layout
//! ```text
//! ┌────────────────────────────────────────┐
//! │  Block Header                          │
//! │  ├── height       (u64)                │
//! │  ├── prev_hash    ([u8;32])            │
//! │  ├── tx_root      ([u8;32])  ← Merkle │
//! │  ├── state_root   ([u8;32])            │
//! │  ├── timestamp    (u64)                │
//! │  ├── difficulty   (u32)                │
//! │  └── nonce        (u64)                │
//! ├────────────────────────────────────────┤
//! │  Transactions  [Transaction; 0..MAX]   │
//! └────────────────────────────────────────┘
//! ```

use crate::transaction::Transaction;

// ─── SHA-256 (inlined) ────────────────────────────────────────────────────────

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
    for (i,&word) in h.iter().enumerate() {
        out[i*4..(i+1)*4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

fn sha256d(data: &[u8]) -> [u8; 32] { sha256(&sha256(data)) }

// ─── Constants ────────────────────────────────────────────────────────────────

/// Maximum number of transactions per block.
pub const MAX_TXS_PER_BLOCK: usize = 4096;

/// Genesis block height.
pub const GENESIS_HEIGHT: u64 = 0;

// ─── Block header ─────────────────────────────────────────────────────────────

/// The header of a Seer block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockHeader {
    /// Block height (0 = genesis).
    pub height: u64,
    /// Hash of the previous block (all-zero for genesis).
    pub prev_hash: [u8; 32],
    /// Merkle root of all transaction hashes in this block.
    pub tx_root: [u8; 32],
    /// State root after applying all transactions.
    pub state_root: [u8; 32],
    /// Unix timestamp of block creation.
    pub timestamp: u64,
    /// Difficulty bits (number of leading zero bits required in block hash).
    pub difficulty: u32,
    /// Nonce found during mining.
    pub nonce: u64,
    /// Address of the miner who produced this block.
    pub miner: [u8; 32],
}

impl BlockHeader {
    /// Serialises the header for hashing (double-SHA-256).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(8+32+32+32+8+4+8+32);
        buf.extend_from_slice(&self.height.to_le_bytes());
        buf.extend_from_slice(&self.prev_hash);
        buf.extend_from_slice(&self.tx_root);
        buf.extend_from_slice(&self.state_root);
        buf.extend_from_slice(&self.timestamp.to_le_bytes());
        buf.extend_from_slice(&self.difficulty.to_le_bytes());
        buf.extend_from_slice(&self.nonce.to_le_bytes());
        buf.extend_from_slice(&self.miner);
        buf
    }

    /// Computes the block hash (double-SHA-256 of header bytes).
    pub fn hash(&self) -> [u8; 32] {
        sha256d(&self.to_bytes())
    }

    /// Returns `true` if the block hash meets the difficulty target.
    pub fn meets_difficulty(&self) -> bool {
        let hash = self.hash();
        let bits = self.difficulty as usize;
        let full_bytes = bits / 8;
        let remainder = bits % 8;
        for &b in hash.iter().take(full_bytes) {
            if b != 0 { return false; }
        }
        if full_bytes < 32 && remainder > 0 {
            let mask = 0xFF_u8 << (8 - remainder);
            if hash[full_bytes] & mask != 0 { return false; }
        }
        true
    }
}

// ─── Merkle tree ──────────────────────────────────────────────────────────────

/// Computes the Merkle root of a list of transaction hashes.
///
/// Uses a standard binary Merkle tree with SHA-256 at each node.
/// An empty list returns the all-zero hash.
pub fn merkle_root(tx_hashes: &[[u8; 32]]) -> [u8; 32] {
    if tx_hashes.is_empty() {
        return [0u8; 32];
    }
    let mut layer: Vec<[u8; 32]> = tx_hashes.to_vec();
    while layer.len() > 1 {
        // Duplicate last element if odd count (standard Bitcoin convention).
        if layer.len() % 2 == 1 {
            let last = *layer.last().unwrap();
            layer.push(last);
        }
        let mut next = Vec::with_capacity(layer.len() / 2);
        for pair in layer.chunks(2) {
            let mut combined = [0u8; 64];
            combined[..32].copy_from_slice(&pair[0]);
            combined[32..].copy_from_slice(&pair[1]);
            next.push(sha256d(&combined));
        }
        layer = next;
    }
    layer[0]
}

// ─── Block ────────────────────────────────────────────────────────────────────

/// A complete Seer block: header + transactions.
#[derive(Debug, Clone)]
pub struct Block {
    /// The block header.
    pub header: BlockHeader,
    /// Transactions included in this block (coinbase first).
    pub transactions: Vec<Transaction>,
}

impl Block {
    /// Creates a new block and computes the Merkle root from transactions.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        height: u64,
        prev_hash: [u8; 32],
        state_root: [u8; 32],
        timestamp: u64,
        difficulty: u32,
        nonce: u64,
        miner: [u8; 32],
        transactions: Vec<Transaction>,
    ) -> Self {
        let tx_hashes: Vec<[u8; 32]> = transactions.iter().map(|tx| *tx.hash()).collect();
        let tx_root = merkle_root(&tx_hashes);
        Block {
            header: BlockHeader {
                height,
                prev_hash,
                tx_root,
                state_root,
                timestamp,
                difficulty,
                nonce,
                miner,
            },
            transactions,
        }
    }

    /// Returns the block hash.
    pub fn hash(&self) -> [u8; 32] {
        self.header.hash()
    }

    /// Returns the block height.
    pub fn height(&self) -> u64 {
        self.header.height
    }

    /// Returns `true` if this is the genesis block.
    pub fn is_genesis(&self) -> bool {
        self.header.height == GENESIS_HEIGHT && self.header.prev_hash == [0u8; 32]
    }

    /// Returns the total fees collected in this block.
    pub fn total_fees(&self) -> u64 {
        self.transactions
            .iter()
            .filter(|tx| !tx.is_coinbase())
            .map(|tx| tx.fee)
            .sum()
    }

    /// Returns the total tokens burned in this block.
    pub fn total_burned(&self) -> u64 {
        self.transactions.iter().map(|tx| tx.burned).sum()
    }

    /// Returns the coinbase transaction, if any.
    pub fn coinbase(&self) -> Option<&Transaction> {
        self.transactions.iter().find(|tx| tx.is_coinbase())
    }

    /// Returns the number of non-coinbase transactions.
    pub fn tx_count(&self) -> usize {
        self.transactions.iter().filter(|tx| !tx.is_coinbase()).count()
    }
}

// ─── Block validation ─────────────────────────────────────────────────────────

/// Errors from block validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockError {
    /// Block hash does not meet the difficulty target.
    InsufficientWork,
    /// Merkle root in header does not match computed root.
    MerkleRootMismatch,
    /// Block exceeds the maximum transaction count.
    TooManyTransactions(usize),
    /// Block has no coinbase transaction.
    MissingCoinbase,
    /// Block has more than one coinbase transaction.
    DuplicateCoinbase,
    /// Coinbase is not the first transaction.
    CoinbaseNotFirst,
    /// Previous hash does not match the expected value.
    PrevHashMismatch,
    /// Block height is not one more than the previous block height.
    InvalidHeight { expected: u64, got: u64 },
}

impl std::fmt::Display for BlockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlockError::InsufficientWork => write!(f, "block hash does not meet difficulty"),
            BlockError::MerkleRootMismatch => write!(f, "Merkle root mismatch"),
            BlockError::TooManyTransactions(n) => write!(f, "too many transactions: {n}"),
            BlockError::MissingCoinbase => write!(f, "block has no coinbase transaction"),
            BlockError::DuplicateCoinbase => write!(f, "block has multiple coinbase transactions"),
            BlockError::CoinbaseNotFirst => write!(f, "coinbase is not the first transaction"),
            BlockError::PrevHashMismatch => write!(f, "prev_hash does not match parent block"),
            BlockError::InvalidHeight { expected, got } =>
                write!(f, "invalid height: expected {expected}, got {got}"),
        }
    }
}

/// Validates a block's structural integrity.
///
/// Does NOT validate transaction signatures or balances — that is the ledger's
/// responsibility. This function validates:
/// - PoW difficulty
/// - Merkle root
/// - Transaction count
/// - Coinbase presence and position
pub fn validate_block(block: &Block) -> Result<(), BlockError> {
    // PoW check.
    if !block.header.meets_difficulty() {
        return Err(BlockError::InsufficientWork);
    }

    // Transaction count.
    if block.transactions.len() > MAX_TXS_PER_BLOCK {
        return Err(BlockError::TooManyTransactions(block.transactions.len()));
    }

    // Coinbase checks.
    let coinbase_count = block.transactions.iter().filter(|tx| tx.is_coinbase()).count();
    if coinbase_count == 0 {
        return Err(BlockError::MissingCoinbase);
    }
    if coinbase_count > 1 {
        return Err(BlockError::DuplicateCoinbase);
    }
    if !block.transactions.first().map(|tx| tx.is_coinbase()).unwrap_or(false) {
        return Err(BlockError::CoinbaseNotFirst);
    }

    // Merkle root.
    let tx_hashes: Vec<[u8; 32]> = block.transactions.iter().map(|tx| *tx.hash()).collect();
    let computed_root = merkle_root(&tx_hashes);
    if computed_root != block.header.tx_root {
        return Err(BlockError::MerkleRootMismatch);
    }

    Ok(())
}

/// Validates a block's linkage to its parent.
pub fn validate_block_linkage(
    block: &Block,
    parent_hash: &[u8; 32],
    parent_height: u64,
) -> Result<(), BlockError> {
    if block.header.prev_hash != *parent_hash {
        return Err(BlockError::PrevHashMismatch);
    }
    let expected_height = parent_height + 1;
    if block.header.height != expected_height {
        return Err(BlockError::InvalidHeight {
            expected: expected_height,
            got: block.header.height,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transaction::TxKind;

    fn make_coinbase(height: u64) -> Transaction {
        Transaction::coinbase([0xAAu8; 32], 5_000_000_000, height)
    }

    fn make_block_with_difficulty(difficulty: u32) -> Block {
        // Mine a valid nonce at the given difficulty.
        let coinbase = make_coinbase(1);
        let txs = vec![coinbase];
        let tx_hashes: Vec<[u8; 32]> = txs.iter().map(|tx| *tx.hash()).collect();
        let tx_root = merkle_root(&tx_hashes);
        let state_root = [0u8; 32];
        let miner = [0xBBu8; 32];

        for nonce in 0u64..10_000_000 {
            let header = BlockHeader {
                height: 1,
                prev_hash: [0u8; 32],
                tx_root,
                state_root,
                timestamp: 1_700_000_000,
                difficulty,
                nonce,
                miner,
            };
            if header.meets_difficulty() {
                return Block {
                    header,
                    transactions: txs,
                };
            }
        }
        panic!("could not mine block at difficulty {difficulty} within 10M nonces");
    }

    #[test]
    fn test_merkle_root_empty() {
        assert_eq!(merkle_root(&[]), [0u8; 32]);
    }

    #[test]
    fn test_merkle_root_single() {
        let hash = [1u8; 32];
        // Single element: the while loop condition (len > 1) is false,
        // so the root is the element itself.
        assert_eq!(merkle_root(&[hash]), hash);
    }

    #[test]
    fn test_merkle_root_two_elements() {
        let h1 = [1u8; 32];
        let h2 = [2u8; 32];
        let mut combined = [0u8; 64];
        combined[..32].copy_from_slice(&h1);
        combined[32..].copy_from_slice(&h2);
        let expected = sha256d(&combined);
        assert_eq!(merkle_root(&[h1, h2]), expected);
    }

    #[test]
    fn test_block_hash_deterministic() {
        let block = make_block_with_difficulty(1);
        assert_eq!(block.hash(), block.hash());
    }

    #[test]
    fn test_validate_block_ok() {
        let block = make_block_with_difficulty(1);
        assert!(validate_block(&block).is_ok(), "valid block should pass");
    }

    #[test]
    fn test_validate_block_missing_coinbase() {
        let mut block = make_block_with_difficulty(1);
        block.transactions.clear();
        // Recompute tx_root to avoid Merkle mismatch.
        block.header.tx_root = merkle_root(&[]);
        assert_eq!(validate_block(&block), Err(BlockError::MissingCoinbase));
    }

    #[test]
    fn test_validate_block_merkle_mismatch() {
        let mut block = make_block_with_difficulty(1);
        block.header.tx_root = [0xFFu8; 32]; // corrupt the root
        assert_eq!(validate_block(&block), Err(BlockError::MerkleRootMismatch));
    }

    #[test]
    fn test_block_linkage_ok() {
        let parent_hash = [0u8; 32];
        let block = make_block_with_difficulty(1);
        assert!(validate_block_linkage(&block, &parent_hash, 0).is_ok());
    }

    #[test]
    fn test_block_linkage_wrong_prev_hash() {
        let block = make_block_with_difficulty(1);
        let wrong_parent = [0xFFu8; 32];
        assert_eq!(
            validate_block_linkage(&block, &wrong_parent, 0),
            Err(BlockError::PrevHashMismatch)
        );
    }

    #[test]
    fn test_block_linkage_wrong_height() {
        let block = make_block_with_difficulty(1);
        assert_eq!(
            validate_block_linkage(&block, &[0u8; 32], 5),
            Err(BlockError::InvalidHeight { expected: 6, got: 1 })
        );
    }

    #[test]
    fn test_block_total_fees() {
        let block = make_block_with_difficulty(1);
        // Only a coinbase, so fees = 0.
        assert_eq!(block.total_fees(), 0);
    }

    #[test]
    fn test_genesis_detection() {
        let coinbase = make_coinbase(0);
        let block = Block::new(0, [0u8; 32], [0u8; 32], 0, 0, 0, [0u8; 32], vec![coinbase]);
        assert!(block.is_genesis());
    }
}
