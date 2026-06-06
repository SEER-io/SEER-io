//! # Seer Node Factory Bot
//!
//! Implements the Node Factory Bot bootstrap script. The factory bot:
//! - Generates ADNL key pairs for new nodes.
//! - Processes `NODE_REGISTER` actions from operators.
//! - Ships genesis snapshots to newly registered nodes.
//! - Maintains a registry of all known nodes in the network.
//!
//! ## Registration Flow
//! ```text
//! Operator → FACTORY: NODE_REGISTER {seed_hex}
//! FACTORY  → Operator: NODE_CREATED {node_id_hex} {address}
//! FACTORY  → NewNode:  GENESIS_SNAPSHOT {genesis_bytes_hex}
//! ```

use std::collections::HashMap;
use crate::transport::{TgMessage, TgMessageKind};

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
        for i in 0..16 { w[i] = u32::from_be_bytes([chunk[i*4],chunk[i*4+1],chunk[i*4+2],chunk[i*4+3]]); }
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

// ─── Node registration record ─────────────────────────────────────────────────

/// A registered node entry in the factory registry.
#[derive(Debug, Clone)]
pub struct NodeRecord {
    /// The node's ADNL identity (SHA-256 of public key).
    pub node_id: [u8; 32],
    /// The node's public key bytes.
    pub public_key: [u8; 32],
    /// The node's network address (host:port).
    pub address: String,
    /// Unix timestamp of registration.
    pub registered_at: u64,
    /// Whether the node has received its genesis snapshot.
    pub genesis_shipped: bool,
}

// ─── Factory bot ──────────────────────────────────────────────────────────────

/// The Seer Node Factory Bot.
#[derive(Debug)]
pub struct FactoryBot {
    /// Factory bot's own ADNL identity.
    pub factory_id: [u8; 32],
    /// Registry of all registered nodes.
    nodes: HashMap<[u8; 32], NodeRecord>,
    /// The genesis block bytes (to ship to new nodes).
    genesis_bytes: Vec<u8>,
    /// Outbound message queue.
    outbound: std::collections::VecDeque<TgMessage>,
    /// Sequence counter.
    seq: u64,
    /// Current time.
    pub current_time: u64,
}

impl FactoryBot {
    /// Creates a new factory bot.
    pub fn new(factory_id: [u8; 32], genesis_bytes: Vec<u8>, current_time: u64) -> Self {
        FactoryBot {
            factory_id,
            nodes: HashMap::new(),
            genesis_bytes,
            outbound: std::collections::VecDeque::new(),
            seq: 0,
            current_time,
        }
    }

    /// Processes a `NODE_REGISTER` command.
    ///
    /// `seed` is the 32-byte key seed for the new node.
    /// `address` is the node's network address string.
    ///
    /// Returns the new node's ADNL identity on success.
    pub fn register_node(&mut self, seed: [u8; 32], address: String) -> Result<[u8; 32], FactoryError> {
        // Derive public key: SHA-256(seed ++ "pubkey")
        let mut pk_input = seed.to_vec();
        pk_input.extend_from_slice(b"pubkey");
        let public_key: [u8; 32] = sha256(&pk_input).into();

        // Derive ADNL identity: SHA-256(public_key)
        let node_id: [u8; 32] = sha256(&public_key).into();

        if self.nodes.contains_key(&node_id) {
            return Err(FactoryError::AlreadyRegistered);
        }

        let record = NodeRecord {
            node_id,
            public_key,
            address: address.clone(),
            registered_at: self.current_time,
            genesis_shipped: false,
        };
        self.nodes.insert(node_id, record);

        // Enqueue NODE_CREATED confirmation.
        let mut payload = node_id.to_vec();
        payload.extend_from_slice(address.as_bytes());
        let msg = TgMessage::new(TgMessageKind::Pong, &payload, self.factory_id, self.next_seq());
        self.outbound.push_back(msg);

        Ok(node_id)
    }

    /// Ships the genesis snapshot to a registered node.
    pub fn ship_genesis(&mut self, node_id: &[u8; 32]) -> Result<(), FactoryError> {
        let record = self.nodes.get_mut(node_id)
            .ok_or(FactoryError::NodeNotFound)?;
        if record.genesis_shipped {
            return Err(FactoryError::GenesisAlreadyShipped);
        }
        record.genesis_shipped = true;
        // Enqueue genesis snapshot message.
        let msg = TgMessage::new(
            TgMessageKind::Block,
            &self.genesis_bytes.clone(),
            self.factory_id,
            self.next_seq(),
        );
        self.outbound.push_back(msg);
        Ok(())
    }

    /// Returns the next outbound message.
    pub fn pop_outbound(&mut self) -> Option<TgMessage> {
        self.outbound.pop_front()
    }

    /// Returns the number of registered nodes.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Returns a node record by ADNL identity.
    pub fn get_node(&self, node_id: &[u8; 32]) -> Option<&NodeRecord> {
        self.nodes.get(node_id)
    }

    fn next_seq(&mut self) -> u64 {
        let s = self.seq;
        self.seq += 1;
        s
    }
}

/// Errors from factory operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FactoryError {
    /// A node with this identity is already registered.
    AlreadyRegistered,
    /// No node with this identity was found.
    NodeNotFound,
    /// Genesis snapshot has already been shipped to this node.
    GenesisAlreadyShipped,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_factory() -> FactoryBot {
        FactoryBot::new([0xFFu8; 32], vec![0xABu8; 128], 1_700_000_000)
    }

    #[test]
    fn test_register_node() {
        let mut factory = make_factory();
        let node_id = factory.register_node([0x01u8; 32], "10.0.0.1:443".to_string()).unwrap();
        assert_eq!(factory.node_count(), 1);
        assert!(factory.get_node(&node_id).is_some());
    }

    #[test]
    fn test_register_duplicate_fails() {
        let mut factory = make_factory();
        factory.register_node([0x01u8; 32], "10.0.0.1:443".to_string()).unwrap();
        let err = factory.register_node([0x01u8; 32], "10.0.0.1:443".to_string()).unwrap_err();
        assert_eq!(err, FactoryError::AlreadyRegistered);
    }

    #[test]
    fn test_ship_genesis() {
        let mut factory = make_factory();
        let node_id = factory.register_node([0x02u8; 32], "10.0.0.2:443".to_string()).unwrap();
        factory.ship_genesis(&node_id).unwrap();
        assert!(factory.get_node(&node_id).unwrap().genesis_shipped);
    }

    #[test]
    fn test_ship_genesis_twice_fails() {
        let mut factory = make_factory();
        let node_id = factory.register_node([0x03u8; 32], "10.0.0.3:443".to_string()).unwrap();
        factory.ship_genesis(&node_id).unwrap();
        let err = factory.ship_genesis(&node_id).unwrap_err();
        assert_eq!(err, FactoryError::GenesisAlreadyShipped);
    }

    #[test]
    fn test_ship_genesis_unknown_node_fails() {
        let mut factory = make_factory();
        let err = factory.ship_genesis(&[0xAAu8; 32]).unwrap_err();
        assert_eq!(err, FactoryError::NodeNotFound);
    }
}
