//! # Seer Bot Node
//!
//! The primary logic daemon for standard validation and mining bot instances
//! (`@projectname_node_<id>_bot`). Each bot node:
//! - Listens to the master channel for new block and transaction announcements.
//! - Validates incoming messages using the transport layer.
//! - Maintains a local mempool of pending transactions.
//! - Attempts to mine new blocks and broadcasts them to the network.
//! - Responds to PING messages with PONG.

use crate::transport::{TgMessage, TgMessageKind};
use std::collections::{HashMap, VecDeque};

// ─── Constants ────────────────────────────────────────────────────────────────

/// Maximum number of messages to buffer in the inbound queue.
pub const MAX_INBOUND_QUEUE: usize = 1024;

/// Maximum number of pending transactions in the local mempool.
pub const MAX_MEMPOOL_SIZE: usize = 4096;

// ─── Node state ───────────────────────────────────────────────────────────────

/// The operational state of a bot node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeState {
    /// Node is starting up and syncing with the network.
    Syncing,
    /// Node is fully synced and participating in consensus.
    Active,
    /// Node has been paused (e.g., awaiting operator intervention).
    Paused,
    /// Node has encountered a fatal error and stopped.
    Faulted { reason: String },
}

// ─── Mempool entry ────────────────────────────────────────────────────────────

/// A pending transaction in the local mempool.
#[derive(Debug, Clone)]
pub struct MempoolEntry {
    /// Raw transaction bytes (hex-decoded from transport message).
    pub tx_bytes: Vec<u8>,
    /// The transaction hash (32 bytes).
    pub tx_hash: [u8; 32],
    /// Fee extracted from the transaction (for priority ordering).
    pub fee: u64,
    /// Timestamp when the transaction was received.
    pub received_at: u64,
}

// ─── Bot node ─────────────────────────────────────────────────────────────────

/// A Seer validation and mining bot node.
#[derive(Debug)]
pub struct BotNode {
    /// Unique node identifier (ADNL identity bytes).
    pub node_id: [u8; 32],
    /// Human-readable node name (e.g., "seer_node_001_bot").
    pub name: String,
    /// Current operational state.
    pub state: NodeState,
    /// Current chain tip hash.
    pub tip_hash: [u8; 32],
    /// Current chain tip height.
    pub tip_height: u64,
    /// Local mempool: tx_hash → entry.
    mempool: HashMap<[u8; 32], MempoolEntry>,
    /// Inbound message queue (FIFO).
    inbound: VecDeque<TgMessage>,
    /// Outbound message queue (FIFO).
    outbound: VecDeque<TgMessage>,
    /// Sequence counter for outbound messages.
    seq: u64,
    /// Simulated current time (seconds since epoch).
    pub current_time: u64,
}

impl BotNode {
    /// Creates a new bot node.
    pub fn new(node_id: [u8; 32], name: String, current_time: u64) -> Self {
        BotNode {
            node_id,
            name,
            state: NodeState::Syncing,
            tip_hash: [0u8; 32],
            tip_height: 0,
            mempool: HashMap::new(),
            inbound: VecDeque::new(),
            outbound: VecDeque::new(),
            seq: 0,
            current_time,
        }
    }

    /// Transitions the node to the Active state.
    pub fn activate(&mut self) {
        self.state = NodeState::Active;
    }

    /// Pauses the node.
    pub fn pause(&mut self) {
        self.state = NodeState::Paused;
    }

    /// Enqueues an inbound message for processing.
    ///
    /// Drops messages if the queue is full (backpressure).
    pub fn receive(&mut self, msg: TgMessage) -> bool {
        if self.inbound.len() >= MAX_INBOUND_QUEUE {
            return false;
        }
        self.inbound.push_back(msg);
        true
    }

    /// Processes all messages currently in the inbound queue.
    ///
    /// Returns the number of messages processed.
    pub fn process_inbound(&mut self) -> usize {
        let mut count = 0;
        while let Some(msg) = self.inbound.pop_front() {
            self.handle_message(msg);
            count += 1;
        }
        count
    }

    /// Handles a single inbound message.
    fn handle_message(&mut self, msg: TgMessage) {
        match msg.kind {
            TgMessageKind::Tx => self.handle_tx(msg),
            TgMessageKind::Block => self.handle_block(msg),
            TgMessageKind::Ping => self.handle_ping(msg),
            TgMessageKind::Inv => self.handle_inv(msg),
            _ => {} // Ignore PONG, PATCH, etc. at this layer
        }
    }

    fn handle_tx(&mut self, msg: TgMessage) {
        if self.mempool.len() >= MAX_MEMPOOL_SIZE {
            return; // Drop when full
        }
        if let Ok(bytes) = msg.payload_bytes() {
            if bytes.len() < 32 { return; }
            let mut hash = [0u8; 32];
            hash.copy_from_slice(&bytes[..32]);
            if self.mempool.contains_key(&hash) { return; } // Deduplicate
            // Extract fee from bytes[32..40] if available (simplified).
            let fee = if bytes.len() >= 40 {
                u64::from_le_bytes(bytes[32..40].try_into().unwrap_or([0u8; 8]))
            } else { 0 };
            self.mempool.insert(hash, MempoolEntry {
                tx_bytes: bytes,
                tx_hash: hash,
                fee,
                received_at: self.current_time,
            });
        }
    }

    fn handle_block(&mut self, msg: TgMessage) {
        if let Ok(bytes) = msg.payload_bytes() {
            if bytes.len() < 40 { return; }
            // Extract height (bytes 0..8) and hash (bytes 8..40).
            let height = u64::from_le_bytes(bytes[0..8].try_into().unwrap_or([0u8; 8]));
            let mut hash = [0u8; 32];
            hash.copy_from_slice(&bytes[8..40]);
            if height > self.tip_height {
                self.tip_height = height;
                self.tip_hash = hash;
                // Evict mempool entries that were included (simplified: clear 10%).
                let to_remove: Vec<[u8; 32]> = self.mempool.keys()
                    .take(self.mempool.len() / 10)
                    .copied()
                    .collect();
                for k in to_remove { self.mempool.remove(&k); }
            }
        }
    }

    fn handle_ping(&mut self, _msg: TgMessage) {
        let pong = TgMessage::new(
            TgMessageKind::Pong,
            &self.node_id.clone(),
            self.node_id,
            self.next_seq(),
        );
        self.outbound.push_back(pong);
    }

    fn handle_inv(&mut self, _msg: TgMessage) {
        // INV handling: request missing items (simplified — no-op for now).
    }

    /// Returns the next outbound message, if any.
    pub fn pop_outbound(&mut self) -> Option<TgMessage> {
        self.outbound.pop_front()
    }

    /// Returns the number of pending transactions in the mempool.
    pub fn mempool_size(&self) -> usize {
        self.mempool.len()
    }

    /// Returns mempool entries sorted by fee (highest first).
    pub fn mempool_by_fee(&self) -> Vec<&MempoolEntry> {
        let mut entries: Vec<&MempoolEntry> = self.mempool.values().collect();
        entries.sort_unstable_by_key(|b| std::cmp::Reverse(b.fee));
        entries
    }

    fn next_seq(&mut self) -> u64 {
        let s = self.seq;
        self.seq += 1;
        s
    }

    /// Broadcasts a message to the outbound queue.
    pub fn broadcast(&mut self, kind: TgMessageKind, payload: &[u8]) {
        let seq = self.next_seq();
        let msg = TgMessage::new(kind, payload, self.node_id, seq);
        self.outbound.push_back(msg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::TgMessageKind;

    fn make_node() -> BotNode {
        BotNode::new([0x01u8; 32], "test_node".to_string(), 1_700_000_000)
    }

    #[test]
    fn test_node_starts_syncing() {
        let node = make_node();
        assert_eq!(node.state, NodeState::Syncing);
    }

    #[test]
    fn test_activate() {
        let mut node = make_node();
        node.activate();
        assert_eq!(node.state, NodeState::Active);
    }

    #[test]
    fn test_receive_and_process_ping() {
        let mut node = make_node();
        let ping = TgMessage::new(TgMessageKind::Ping, &[0x01u8; 32], [0x02u8; 32], 0);
        node.receive(ping);
        node.process_inbound();
        let pong = node.pop_outbound().expect("should have a PONG response");
        assert_eq!(pong.kind, TgMessageKind::Pong);
    }

    #[test]
    fn test_tx_added_to_mempool() {
        let mut node = make_node();
        let mut payload = vec![0u8; 40];
        payload[..32].copy_from_slice(&[0xAAu8; 32]); // hash
        payload[32..40].copy_from_slice(&5_000u64.to_le_bytes()); // fee
        let msg = TgMessage::new(TgMessageKind::Tx, &payload, [0x02u8; 32], 0);
        node.receive(msg);
        node.process_inbound();
        assert_eq!(node.mempool_size(), 1);
    }

    #[test]
    fn test_duplicate_tx_not_added_twice() {
        let mut node = make_node();
        let mut payload = vec![0u8; 40];
        payload[..32].copy_from_slice(&[0xBBu8; 32]);
        let msg1 = TgMessage::new(TgMessageKind::Tx, &payload, [0x02u8; 32], 0);
        let msg2 = TgMessage::new(TgMessageKind::Tx, &payload, [0x02u8; 32], 1);
        node.receive(msg1);
        node.receive(msg2);
        node.process_inbound();
        assert_eq!(node.mempool_size(), 1);
    }

    #[test]
    fn test_block_updates_tip() {
        let mut node = make_node();
        let mut payload = vec![0u8; 40];
        payload[..8].copy_from_slice(&42u64.to_le_bytes()); // height = 42
        payload[8..40].copy_from_slice(&[0xCCu8; 32]); // hash
        let msg = TgMessage::new(TgMessageKind::Block, &payload, [0x02u8; 32], 0);
        node.receive(msg);
        node.process_inbound();
        assert_eq!(node.tip_height, 42);
    }

    #[test]
    fn test_inbound_queue_backpressure() {
        let mut node = make_node();
        let msg = TgMessage::new(TgMessageKind::Ping, &[0u8; 32], [0x02u8; 32], 0);
        for _ in 0..MAX_INBOUND_QUEUE {
            node.receive(msg.clone());
        }
        // One more should be dropped.
        let accepted = node.receive(msg.clone());
        assert!(!accepted);
    }
}
