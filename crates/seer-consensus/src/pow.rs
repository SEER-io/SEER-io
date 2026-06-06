//! # Seer Proof-of-Work Engine
//!
//! Implements SHA-256 based PoW for block generation, difficulty adjustment,
//! and mining. The engine ensures a fair and decentralised issuance schedule
//! by targeting a fixed block time through a retargeting algorithm.
//!
//! ## Algorithm
//! A block is valid when `SHA256(SHA256(block_header_bytes ++ nonce))` produces
//! a hash whose leading bytes are all zero up to the current difficulty target.
//! This is equivalent to the hash being numerically less than the target value.
//!
//! ## Difficulty Adjustment
//! Every `RETARGET_INTERVAL` blocks the difficulty is recalculated based on
//! the actual time taken to mine the previous epoch versus the expected time.

/// Number of blocks between difficulty retargets (matches Bitcoin's 2016).
pub const RETARGET_INTERVAL: u64 = 2016;

/// Target block time in seconds (from genesis.toml).
pub const TARGET_BLOCK_TIME_SECS: u64 = 10;

/// Maximum difficulty adjustment factor per retarget (4× up or down).
pub const MAX_ADJUSTMENT_FACTOR: u64 = 4;

/// Initial difficulty: number of leading zero bits required in a valid hash.
pub const INITIAL_DIFFICULTY_BITS: u32 = 16;

/// A 32-byte hash alias.
pub type Hash32 = [u8; 32];

// ─── Difficulty ───────────────────────────────────────────────────────────────

/// Encodes the current mining difficulty as a compact target value.
///
/// The target is represented as the number of leading zero bits required in
/// a valid block hash. Higher values mean harder mining.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Difficulty(pub u32);

impl Difficulty {
    /// Creates a difficulty from a leading-zero-bit count.
    pub fn from_bits(bits: u32) -> Self {
        Difficulty(bits.min(255))
    }

    /// Returns the number of leading zero bits required.
    pub fn bits(&self) -> u32 {
        self.0
    }

    /// Converts the difficulty to a 32-byte target threshold.
    ///
    /// A hash is valid if it is numerically less than this target.
    pub fn to_target(&self) -> [u8; 32] {
        let mut target = [0xFFu8; 32];
        let bits = self.0 as usize;
        let full_bytes = bits / 8;
        let remainder = bits % 8;
        for b in target.iter_mut().take(full_bytes) {
            *b = 0x00;
        }
        if full_bytes < 32 {
            target[full_bytes] = 0xFF >> remainder;
        }
        target
    }

    /// Returns `true` if the given hash meets this difficulty target.
    pub fn is_met_by(&self, hash: &Hash32) -> bool {
        let target = self.to_target();
        *hash < target
    }
}

impl Default for Difficulty {
    fn default() -> Self {
        Difficulty(INITIAL_DIFFICULTY_BITS)
    }
}

// ─── Block header (minimal, for hashing purposes) ─────────────────────────────

/// The fields of a block header that are included in the PoW hash input.
#[derive(Debug, Clone)]
pub struct BlockHeader {
    /// Block height.
    pub height: u64,
    /// Hash of the previous block.
    pub prev_hash: Hash32,
    /// Merkle root of all transactions in this block.
    pub tx_root: Hash32,
    /// Unix timestamp of block creation.
    pub timestamp: u64,
    /// Current difficulty bits.
    pub difficulty: Difficulty,
    /// The nonce found during mining.
    pub nonce: u64,
}

impl BlockHeader {
    /// Serialises the header fields (excluding nonce) into bytes for hashing.
    pub fn to_bytes_without_nonce(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(8 + 32 + 32 + 8 + 4);
        buf.extend_from_slice(&self.height.to_le_bytes());
        buf.extend_from_slice(&self.prev_hash);
        buf.extend_from_slice(&self.tx_root);
        buf.extend_from_slice(&self.timestamp.to_le_bytes());
        buf.extend_from_slice(&self.difficulty.0.to_le_bytes());
        buf
    }

    /// Serialises the full header (including nonce) into bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = self.to_bytes_without_nonce();
        buf.extend_from_slice(&self.nonce.to_le_bytes());
        buf
    }
}

// ─── SHA-256 (pure Rust, no external crate) ───────────────────────────────────

/// Computes SHA-256 over the input bytes using a pure-Rust implementation.
///
/// This avoids an external dependency while keeping the crate self-contained.
/// Production deployments should replace this with the `sha2` crate for
/// hardware-accelerated performance.
pub fn sha256(data: &[u8]) -> Hash32 {
    // SHA-256 constants: first 32 bits of the fractional parts of the cube
    // roots of the first 64 primes.
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5,
        0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
        0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc,
        0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
        0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3,
        0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5,
        0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
        0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];

    // Initial hash values: first 32 bits of fractional parts of square roots
    // of the first 8 primes.
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
        0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];

    // Pre-processing: pad the message.
    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut msg = data.to_vec();
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0x00);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    // Process each 512-bit (64-byte) chunk.
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([chunk[i*4], chunk[i*4+1], chunk[i*4+2], chunk[i*4+3]]);
        }
        for i in 16..64 {
            let s0 = w[i-15].rotate_right(7) ^ w[i-15].rotate_right(18) ^ (w[i-15] >> 3);
            let s1 = w[i-2].rotate_right(17) ^ w[i-2].rotate_right(19) ^ (w[i-2] >> 10);
            w[i] = w[i-16].wrapping_add(s0).wrapping_add(w[i-7]).wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] =
            [h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]];
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            hh = g; g = f; f = e;
            e = d.wrapping_add(temp1);
            d = c; c = b; b = a;
            a = temp1.wrapping_add(temp2);
        }
        h[0] = h[0].wrapping_add(a); h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c); h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e); h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g); h[7] = h[7].wrapping_add(hh);
    }

    let mut out = [0u8; 32];
    for (i, &word) in h.iter().enumerate() {
        out[i*4..(i+1)*4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

/// Double-SHA-256 (SHA256(SHA256(data))), matching Bitcoin's block hashing.
pub fn sha256d(data: &[u8]) -> Hash32 {
    sha256(&sha256(data))
}

// ─── PoW engine ───────────────────────────────────────────────────────────────

/// Computes the PoW hash for a block header with the given nonce.
pub fn compute_hash(header: &BlockHeader) -> Hash32 {
    sha256d(&header.to_bytes())
}

/// Verifies that a block header's nonce produces a hash meeting the difficulty.
pub fn verify_block(header: &BlockHeader) -> bool {
    let hash = compute_hash(header);
    header.difficulty.is_met_by(&hash)
}

/// Result of a mining attempt.
#[derive(Debug, Clone)]
pub struct MineResult {
    /// The winning nonce.
    pub nonce: u64,
    /// The resulting block hash.
    pub hash: Hash32,
    /// Number of nonces tried before success.
    pub attempts: u64,
}

/// Mines a block by iterating nonces until the difficulty target is met.
///
/// `max_attempts` caps the search to avoid infinite loops in tests. Pass
/// `u64::MAX` for unbounded mining.
///
/// Returns `None` if no valid nonce was found within `max_attempts`.
pub fn mine(header: &mut BlockHeader, max_attempts: u64) -> Option<MineResult> {
    for nonce in 0..max_attempts {
        header.nonce = nonce;
        let hash = compute_hash(header);
        if header.difficulty.is_met_by(&hash) {
            return Some(MineResult { nonce, hash, attempts: nonce + 1 });
        }
    }
    None
}

// ─── Difficulty retargeting ───────────────────────────────────────────────────

/// Recalculates the difficulty for the next epoch.
///
/// `epoch_actual_secs` is the wall-clock time (in seconds) taken to mine the
/// last `RETARGET_INTERVAL` blocks. The new difficulty is clamped to at most
/// `MAX_ADJUSTMENT_FACTOR`× the current value in either direction.
pub fn retarget(current: Difficulty, epoch_actual_secs: u64) -> Difficulty {
    let expected = TARGET_BLOCK_TIME_SECS * RETARGET_INTERVAL;

    // Clamp actual time to [expected/4, expected*4] to limit adjustment.
    let clamped = epoch_actual_secs
        .max(expected / MAX_ADJUSTMENT_FACTOR)
        .min(expected * MAX_ADJUSTMENT_FACTOR);

    // New difficulty scales inversely with actual time.
    // We adjust the bit count: more bits = harder.
    // ratio = expected / clamped  (> 1 means blocks came too fast → increase difficulty)
    // We approximate in integer arithmetic using bit shifts.
    let bits = current.0;
    let new_bits = if clamped < expected {
        // Blocks came too fast: increase difficulty.
        let ratio = expected / clamped.max(1);
        bits.saturating_add(ratio.trailing_zeros())
    } else {
        // Blocks came too slow: decrease difficulty.
        let ratio = clamped / expected.max(1);
        bits.saturating_sub(ratio.trailing_zeros())
    };

    Difficulty::from_bits(new_bits.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_header(difficulty_bits: u32) -> BlockHeader {
        BlockHeader {
            height: 1,
            prev_hash: [0u8; 32],
            tx_root: [0u8; 32],
            timestamp: 1_700_000_000,
            difficulty: Difficulty::from_bits(difficulty_bits),
            nonce: 0,
        }
    }

    #[test]
    fn test_sha256_known_vector() {
        // SHA-256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        let hash = sha256(b"");
        assert_eq!(hash[0], 0xe3);
        assert_eq!(hash[1], 0xb0);
    }

    #[test]
    fn test_sha256_abc() {
        // SHA-256("abc") = ba7816bf8f01cfea414140de5dae2ec73b00361bbef0469f490f4187e0b1807c
        let hash = sha256(b"abc");
        assert_eq!(hash[0], 0xba);
        assert_eq!(hash[1], 0x78);
    }

    #[test]
    fn test_difficulty_target_leading_zeros() {
        let d = Difficulty::from_bits(8);
        let target = d.to_target();
        assert_eq!(target[0], 0x00);
        assert_eq!(target[1], 0xFF);
    }

    #[test]
    fn test_mine_low_difficulty() {
        // Difficulty 1 bit: roughly 50% of hashes will pass — should find quickly.
        let mut header = dummy_header(1);
        let result = mine(&mut header, 1_000);
        assert!(result.is_some(), "should find a nonce within 1000 attempts at difficulty 1");
        let r = result.unwrap();
        assert!(header.difficulty.is_met_by(&r.hash));
    }

    #[test]
    fn test_verify_block_after_mine() {
        let mut header = dummy_header(4);
        mine(&mut header, 100_000).expect("should mine at difficulty 4");
        assert!(verify_block(&header));
    }

    #[test]
    fn test_retarget_too_fast() {
        let current = Difficulty::from_bits(16);
        let expected = TARGET_BLOCK_TIME_SECS * RETARGET_INTERVAL;
        // Blocks came in half the expected time → difficulty should increase.
        let new = retarget(current, expected / 2);
        assert!(new >= current);
    }

    #[test]
    fn test_retarget_too_slow() {
        let current = Difficulty::from_bits(16);
        let expected = TARGET_BLOCK_TIME_SECS * RETARGET_INTERVAL;
        // Blocks took twice as long → difficulty should decrease.
        let new = retarget(current, expected * 2);
        assert!(new <= current);
    }

    #[test]
    fn test_retarget_on_target() {
        let current = Difficulty::from_bits(16);
        let expected = TARGET_BLOCK_TIME_SECS * RETARGET_INTERVAL;
        // Exactly on target → difficulty unchanged.
        let new = retarget(current, expected);
        assert_eq!(new, current);
    }
}
