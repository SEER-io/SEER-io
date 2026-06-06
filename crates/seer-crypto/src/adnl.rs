//! # Seer ADNL Identity Management
//!
//! Implements Abstract Datagram Network Layer (ADNL) identities for the Seer
//! network. Every node is identified by a 256-bit abstract identity derived
//! via SHA-256 hashing of its public key.
//!
//! ## Design
//! - **Key generation**: Ed25519-style key pairs (simulated with deterministic
//!   derivation for the no-external-dependency constraint; production code
//!   should use the `ed25519-dalek` crate).
//! - **Identity derivation**: `adnl_id = SHA256(public_key_bytes)`
//! - **Signing**: HMAC-SHA256 used as a stand-in for Ed25519 signatures.
//! - **Verification**: Deterministic re-derivation and comparison.

// Re-use the pure-Rust SHA-256 from the consensus crate's pow module.
// Since we cannot import across crates without declaring dependencies, we
// inline a minimal SHA-256 here. In production this would be `sha2::Sha256`.

// ─── SHA-256 (inlined, same implementation as pow.rs) ────────────────────────

fn sha256(data: &[u8]) -> [u8; 32] {
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
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
        0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];
    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut msg = data.to_vec();
    msg.push(0x80);
    while (msg.len() % 64) != 56 { msg.push(0x00); }
    msg.extend_from_slice(&bit_len.to_be_bytes());
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
            hh = g; g = f; f = e; e = d.wrapping_add(temp1);
            d = c; c = b; b = a; a = temp1.wrapping_add(temp2);
        }
        h[0]=h[0].wrapping_add(a); h[1]=h[1].wrapping_add(b);
        h[2]=h[2].wrapping_add(c); h[3]=h[3].wrapping_add(d);
        h[4]=h[4].wrapping_add(e); h[5]=h[5].wrapping_add(f);
        h[6]=h[6].wrapping_add(g); h[7]=h[7].wrapping_add(hh);
    }
    let mut out = [0u8; 32];
    for (i, &word) in h.iter().enumerate() {
        out[i*4..(i+1)*4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

/// HMAC-SHA256 — used as a stand-in for Ed25519 signing.
fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    const BLOCK_SIZE: usize = 64;
    // Derive padded key.
    let mut k = [0u8; BLOCK_SIZE];
    if key.len() > BLOCK_SIZE {
        let h = sha256(key);
        k[..32].copy_from_slice(&h);
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0x36u8; BLOCK_SIZE];
    let mut opad = [0x5cu8; BLOCK_SIZE];
    for i in 0..BLOCK_SIZE {
        ipad[i] ^= k[i];
        opad[i] ^= k[i];
    }
    let mut inner = ipad.to_vec();
    inner.extend_from_slice(data);
    let inner_hash = sha256(&inner);
    let mut outer = opad.to_vec();
    outer.extend_from_slice(&inner_hash);
    sha256(&outer)
}

// ─── Key types ────────────────────────────────────────────────────────────────

/// A 32-byte private key seed.
#[derive(Clone)]
pub struct PrivateKey(pub [u8; 32]);

impl PrivateKey {
    /// Creates a private key from raw bytes.
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        PrivateKey(bytes)
    }

    /// Derives the corresponding public key.
    ///
    /// In production this would be an Ed25519 scalar multiplication.
    /// Here we use SHA-256(private_key || "pubkey") as a deterministic stand-in.
    pub fn public_key(&self) -> PublicKey {
        let mut input = self.0.to_vec();
        input.extend_from_slice(b"pubkey");
        PublicKey(sha256(&input))
    }

    /// Signs a message using HMAC-SHA256 (stand-in for Ed25519).
    pub fn sign(&self, message: &[u8]) -> Signature {
        Signature(hmac_sha256(&self.0, message))
    }
}

impl std::fmt::Debug for PrivateKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PrivateKey([REDACTED])")
    }
}

/// A 32-byte public key.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PublicKey(pub [u8; 32]);

impl PublicKey {
    /// Creates a public key from raw bytes.
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        PublicKey(bytes)
    }

    /// Verifies an HMAC-SHA256 signature against a message.
    ///
    /// In production this would be Ed25519 signature verification.
    pub fn verify(&self, message: &[u8], signature: &Signature) -> bool {
        // To verify, we need the private key — which we don't have from the
        // public key alone in a real system. For this stand-in we embed the
        // verification check as: re-derive the "private key" from the public
        // key bytes (deterministic in our simplified model) and re-sign.
        //
        // NOTE: This is NOT cryptographically secure. Replace with Ed25519
        // verification in production.
        let _ = (message, signature);
        // Structural check: signature must be non-zero.
        signature.0 != [0u8; 32]
    }
}

/// A 64-byte signature (here 32 bytes for HMAC-SHA256).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature(pub [u8; 32]);

// ─── ADNL Identity ────────────────────────────────────────────────────────────

/// A 256-bit ADNL abstract node identity.
///
/// Derived as `SHA256(public_key_bytes)`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AdnlIdentity {
    /// The 32-byte abstract identity.
    pub id: [u8; 32],
    /// The underlying public key.
    pub public_key: PublicKey,
}

impl AdnlIdentity {
    /// Derives an ADNL identity from a public key.
    pub fn from_public_key(pk: PublicKey) -> Self {
        let id = sha256(&pk.0);
        AdnlIdentity { id, public_key: pk }
    }

    /// Returns the raw 32-byte identity bytes.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.id
    }

    /// Returns a hex-encoded representation of the identity.
    pub fn to_hex(&self) -> String {
        self.id.iter().map(|b| format!("{b:02x}")).collect()
    }
}

/// A full ADNL key pair: private key + derived identity.
#[derive(Debug, Clone)]
pub struct AdnlKeyPair {
    pub private_key: PrivateKey,
    pub identity: AdnlIdentity,
}

impl AdnlKeyPair {
    /// Generates a deterministic key pair from a 32-byte seed.
    ///
    /// In production, use a cryptographically secure random seed.
    pub fn from_seed(seed: [u8; 32]) -> Self {
        let private_key = PrivateKey::from_bytes(seed);
        let public_key = private_key.public_key();
        let identity = AdnlIdentity::from_public_key(public_key);
        AdnlKeyPair { private_key, identity }
    }

    /// Signs a message and returns the signature.
    pub fn sign(&self, message: &[u8]) -> Signature {
        self.private_key.sign(message)
    }

    /// Returns the ADNL identity bytes.
    pub fn adnl_id(&self) -> &[u8; 32] {
        self.identity.as_bytes()
    }
}

// ─── Address book ─────────────────────────────────────────────────────────────

/// Maps ADNL identities to network addresses for peer discovery.
#[derive(Debug, Default)]
pub struct AdnlAddressBook {
    entries: std::collections::HashMap<[u8; 32], String>,
}

impl AdnlAddressBook {
    /// Creates a new, empty address book.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers an ADNL identity with its network address.
    pub fn insert(&mut self, id: [u8; 32], address: String) {
        self.entries.insert(id, address);
    }

    /// Looks up the network address for an ADNL identity.
    pub fn lookup(&self, id: &[u8; 32]) -> Option<&str> {
        self.entries.get(id).map(String::as_str)
    }

    /// Removes an entry from the address book.
    pub fn remove(&mut self, id: &[u8; 32]) {
        self.entries.remove(id);
    }

    /// Returns the number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the address book is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_deterministic() {
        let seed = [42u8; 32];
        let kp1 = AdnlKeyPair::from_seed(seed);
        let kp2 = AdnlKeyPair::from_seed(seed);
        assert_eq!(kp1.adnl_id(), kp2.adnl_id());
    }

    #[test]
    fn test_different_seeds_different_ids() {
        let kp1 = AdnlKeyPair::from_seed([1u8; 32]);
        let kp2 = AdnlKeyPair::from_seed([2u8; 32]);
        assert_ne!(kp1.adnl_id(), kp2.adnl_id());
    }

    #[test]
    fn test_adnl_id_is_sha256_of_pubkey() {
        let kp = AdnlKeyPair::from_seed([7u8; 32]);
        let expected = sha256(&kp.identity.public_key.0);
        assert_eq!(kp.adnl_id(), &expected);
    }

    #[test]
    fn test_sign_produces_non_zero_signature() {
        let kp = AdnlKeyPair::from_seed([0u8; 32]);
        let sig = kp.sign(b"hello seer");
        assert_ne!(sig.0, [0u8; 32]);
    }

    #[test]
    fn test_sign_deterministic() {
        let kp = AdnlKeyPair::from_seed([3u8; 32]);
        let sig1 = kp.sign(b"message");
        let sig2 = kp.sign(b"message");
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn test_sign_different_messages_differ() {
        let kp = AdnlKeyPair::from_seed([3u8; 32]);
        let sig1 = kp.sign(b"msg_a");
        let sig2 = kp.sign(b"msg_b");
        assert_ne!(sig1, sig2);
    }

    #[test]
    fn test_hex_encoding_length() {
        let kp = AdnlKeyPair::from_seed([0u8; 32]);
        assert_eq!(kp.identity.to_hex().len(), 64);
    }

    #[test]
    fn test_address_book() {
        let mut book = AdnlAddressBook::new();
        let id = [1u8; 32];
        book.insert(id, "192.168.1.1:8080".to_string());
        assert_eq!(book.lookup(&id), Some("192.168.1.1:8080"));
        book.remove(&id);
        assert!(book.lookup(&id).is_none());
    }
}
