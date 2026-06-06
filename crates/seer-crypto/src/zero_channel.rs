//! # Seer Zero-Channel Protocol
//!
//! Implements the zero-channel bootstrapping handshake for secure, decentralised
//! peer discovery. The zero-channel is the initial, unauthenticated channel used
//! by a new node to announce itself and obtain a list of known peers before
//! establishing authenticated ADNL connections.
//!
//! ## Handshake Lifecycle
//! ```text
//! Initiator                        Responder
//!    |                                 |
//!    |--- ZC_HELLO (id, nonce, ts) --> |
//!    |                                 |  (verify nonce freshness)
//!    | <-- ZC_HELLO_ACK (id, nonce) --|
//!    |                                 |  (verify responder identity)
//!    |--- ZC_PEER_REQUEST -----------> |
//!    |                                 |
//!    | <-- ZC_PEER_LIST (contacts) ---|
//!    |                                 |
//!    |--- ZC_CLOSE ------------------> |
//! ```
//!
//! ## Security Properties
//! - Replay protection via timestamp + nonce freshness window.
//! - Identity binding: each message carries the sender's ADNL identity.
//! - No long-term secrets exchanged on the zero-channel; it is intentionally
//!   low-trust and used only for bootstrapping.

use std::collections::HashSet;

// ─── Constants ────────────────────────────────────────────────────────────────

/// Default port for zero-channel connections (from genesis.toml).
pub const ZERO_CHANNEL_PORT: u16 = 443;

/// Maximum age of a HELLO message (in seconds) before it is considered stale.
pub const NONCE_FRESHNESS_WINDOW_SECS: u64 = 30;

/// Maximum number of peers returned in a single ZC_PEER_LIST response.
pub const MAX_PEERS_PER_RESPONSE: usize = 20;

// ─── Message types ────────────────────────────────────────────────────────────

/// A zero-channel protocol message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZcMessage {
    /// Initial greeting from an initiator node.
    Hello {
        /// Sender's ADNL identity.
        sender_id: [u8; 32],
        /// Random 32-byte nonce for replay protection.
        nonce: [u8; 32],
        /// Unix timestamp of message creation.
        timestamp: u64,
    },
    /// Acknowledgement from the responder.
    HelloAck {
        /// Responder's ADNL identity.
        responder_id: [u8; 32],
        /// Echo of the initiator's nonce (proves freshness).
        echo_nonce: [u8; 32],
        /// Responder's own nonce for the reverse direction.
        responder_nonce: [u8; 32],
    },
    /// Request for a list of known peers.
    PeerRequest {
        sender_id: [u8; 32],
    },
    /// Response containing a list of known peer addresses.
    PeerList {
        peers: Vec<PeerInfo>,
    },
    /// Graceful close of the zero-channel session.
    Close {
        sender_id: [u8; 32],
    },
}

/// A peer entry returned in a `PeerList` message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerInfo {
    /// The peer's ADNL identity.
    pub adnl_id: [u8; 32],
    /// The peer's network address (host:port).
    pub address: String,
}

// ─── Session state ────────────────────────────────────────────────────────────

/// The state of a zero-channel session from the responder's perspective.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionState {
    /// Waiting for a HELLO message.
    AwaitingHello,
    /// HELLO received and ACK sent; waiting for PEER_REQUEST or CLOSE.
    Established { initiator_id: [u8; 32] },
    /// Session closed (either by CLOSE message or timeout).
    Closed,
}

// ─── Nonce cache (replay protection) ─────────────────────────────────────────

/// Tracks recently seen nonces to prevent replay attacks.
#[derive(Debug, Default)]
pub struct NonceCache {
    seen: HashSet<[u8; 32]>,
}

impl NonceCache {
    /// Creates a new, empty nonce cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` if the nonce has been seen before (replay detected).
    pub fn is_replay(&self, nonce: &[u8; 32]) -> bool {
        self.seen.contains(nonce)
    }

    /// Records a nonce as seen.
    pub fn record(&mut self, nonce: [u8; 32]) {
        self.seen.insert(nonce);
    }

    /// Clears all recorded nonces (called periodically to bound memory usage).
    pub fn flush(&mut self) {
        self.seen.clear();
    }
}

// ─── Handshake processor ──────────────────────────────────────────────────────

/// Errors that can occur during zero-channel processing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZcError {
    /// The message arrived out of order for the current session state.
    UnexpectedMessage,
    /// The HELLO message's timestamp is outside the freshness window.
    StaleTimestamp,
    /// The nonce has been seen before (replay attack).
    ReplayDetected,
    /// The session has already been closed.
    SessionClosed,
    /// The peer list is empty; no peers to return.
    NoPeersAvailable,
}

impl std::fmt::Display for ZcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ZcError::UnexpectedMessage => write!(f, "unexpected message for current state"),
            ZcError::StaleTimestamp => write!(f, "HELLO timestamp outside freshness window"),
            ZcError::ReplayDetected => write!(f, "nonce replay detected"),
            ZcError::SessionClosed => write!(f, "session is already closed"),
            ZcError::NoPeersAvailable => write!(f, "no peers available to return"),
        }
    }
}

/// Processes zero-channel messages on the responder side.
pub struct ZcResponder {
    /// This node's ADNL identity.
    pub local_id: [u8; 32],
    /// Current session state.
    pub state: SessionState,
    /// Nonce replay cache.
    nonce_cache: NonceCache,
    /// Known peers to share with initiators.
    peer_list: Vec<PeerInfo>,
    /// Current simulated wall-clock time (seconds since epoch).
    /// In production this would be `std::time::SystemTime`.
    pub current_time: u64,
}

impl ZcResponder {
    /// Creates a new responder with the given identity and peer list.
    pub fn new(local_id: [u8; 32], peer_list: Vec<PeerInfo>, current_time: u64) -> Self {
        Self {
            local_id,
            state: SessionState::AwaitingHello,
            nonce_cache: NonceCache::new(),
            peer_list,
            current_time,
        }
    }

    /// Processes an incoming zero-channel message and returns the response.
    pub fn process(&mut self, msg: ZcMessage) -> Result<ZcMessage, ZcError> {
        if self.state == SessionState::Closed {
            return Err(ZcError::SessionClosed);
        }

        match msg {
            ZcMessage::Hello { sender_id, nonce, timestamp } => {
                if self.state != SessionState::AwaitingHello {
                    return Err(ZcError::UnexpectedMessage);
                }
                // Freshness check.
                let age = self.current_time.saturating_sub(timestamp);
                if age > NONCE_FRESHNESS_WINDOW_SECS {
                    return Err(ZcError::StaleTimestamp);
                }
                // Replay check.
                if self.nonce_cache.is_replay(&nonce) {
                    return Err(ZcError::ReplayDetected);
                }
                self.nonce_cache.record(nonce);
                // Derive a responder nonce (deterministic from local_id + nonce for testing).
                let mut responder_nonce = [0u8; 32];
                for i in 0..32 {
                    responder_nonce[i] = self.local_id[i] ^ nonce[i];
                }
                self.state = SessionState::Established { initiator_id: sender_id };
                Ok(ZcMessage::HelloAck {
                    responder_id: self.local_id,
                    echo_nonce: nonce,
                    responder_nonce,
                })
            }

            ZcMessage::PeerRequest { sender_id: _ } => {
                match &self.state {
                    SessionState::Established { .. } => {}
                    _ => return Err(ZcError::UnexpectedMessage),
                }
                if self.peer_list.is_empty() {
                    return Err(ZcError::NoPeersAvailable);
                }
                let peers = self
                    .peer_list
                    .iter()
                    .take(MAX_PEERS_PER_RESPONSE)
                    .cloned()
                    .collect();
                Ok(ZcMessage::PeerList { peers })
            }

            ZcMessage::Close { sender_id: _ } => {
                self.state = SessionState::Closed;
                Ok(ZcMessage::Close { sender_id: self.local_id })
            }

            _ => Err(ZcError::UnexpectedMessage),
        }
    }
}

/// Processes zero-channel messages on the initiator side.
pub struct ZcInitiator {
    /// This node's ADNL identity.
    pub local_id: [u8; 32],
    /// The nonce sent in the HELLO message (stored for ACK verification).
    pub sent_nonce: Option<[u8; 32]>,
    /// Current session state.
    pub state: SessionState,
}

impl ZcInitiator {
    /// Creates a new initiator.
    pub fn new(local_id: [u8; 32]) -> Self {
        Self {
            local_id,
            sent_nonce: None,
            state: SessionState::AwaitingHello,
        }
    }

    /// Builds the initial HELLO message with the given nonce and timestamp.
    pub fn build_hello(&mut self, nonce: [u8; 32], timestamp: u64) -> ZcMessage {
        self.sent_nonce = Some(nonce);
        ZcMessage::Hello {
            sender_id: self.local_id,
            nonce,
            timestamp,
        }
    }

    /// Processes a HELLO_ACK from the responder.
    ///
    /// Verifies that the echo nonce matches the sent nonce.
    pub fn process_hello_ack(&mut self, msg: ZcMessage) -> Result<(), ZcError> {
        if let ZcMessage::HelloAck { responder_id, echo_nonce, .. } = msg {
            if let Some(sent) = self.sent_nonce {
                if echo_nonce != sent {
                    return Err(ZcError::ReplayDetected);
                }
            }
            self.state = SessionState::Established { initiator_id: responder_id };
            Ok(())
        } else {
            Err(ZcError::UnexpectedMessage)
        }
    }

    /// Builds a PEER_REQUEST message.
    pub fn build_peer_request(&self) -> ZcMessage {
        ZcMessage::PeerRequest { sender_id: self.local_id }
    }

    /// Builds a CLOSE message.
    pub fn build_close(&mut self) -> ZcMessage {
        self.state = SessionState::Closed;
        ZcMessage::Close { sender_id: self.local_id }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_peer(byte: u8) -> PeerInfo {
        PeerInfo {
            adnl_id: [byte; 32],
            address: format!("10.0.0.{byte}:443"),
        }
    }

    fn setup() -> (ZcInitiator, ZcResponder) {
        let initiator_id = [1u8; 32];
        let responder_id = [2u8; 32];
        let peers = vec![make_peer(3), make_peer(4)];
        let initiator = ZcInitiator::new(initiator_id);
        let responder = ZcResponder::new(responder_id, peers, 1_700_000_000);
        (initiator, responder)
    }

    #[test]
    fn test_full_handshake() {
        let (mut init, mut resp) = setup();
        let nonce = [0xAAu8; 32];
        let ts = 1_700_000_000u64;

        // Step 1: HELLO
        let hello = init.build_hello(nonce, ts);
        let ack = resp.process(hello).unwrap();
        assert!(matches!(ack, ZcMessage::HelloAck { .. }));

        // Step 2: process ACK on initiator side
        init.process_hello_ack(ack).unwrap();
        assert!(matches!(init.state, SessionState::Established { .. }));

        // Step 3: PEER_REQUEST
        let req = init.build_peer_request();
        let peer_list = resp.process(req).unwrap();
        if let ZcMessage::PeerList { peers } = peer_list {
            assert_eq!(peers.len(), 2);
        } else {
            panic!("expected PeerList");
        }

        // Step 4: CLOSE
        let close = init.build_close();
        let close_ack = resp.process(close).unwrap();
        assert!(matches!(close_ack, ZcMessage::Close { .. }));
        assert_eq!(resp.state, SessionState::Closed);
    }

    #[test]
    fn test_stale_timestamp_rejected() {
        let (mut init, mut resp) = setup();
        // Timestamp 60 seconds in the past.
        let ts = resp.current_time - 60;
        let hello = init.build_hello([0xBBu8; 32], ts);
        assert_eq!(resp.process(hello), Err(ZcError::StaleTimestamp));
    }

    #[test]
    fn test_replay_nonce_rejected() {
        let (mut init, mut resp) = setup();
        let nonce = [0xCCu8; 32];
        let ts = resp.current_time;
        let hello1 = init.build_hello(nonce, ts);
        resp.process(hello1).unwrap();
        // Reset state to AwaitingHello to simulate a second HELLO with same nonce.
        resp.state = SessionState::AwaitingHello;
        let hello2 = ZcMessage::Hello { sender_id: [1u8; 32], nonce, timestamp: ts };
        assert_eq!(resp.process(hello2), Err(ZcError::ReplayDetected));
    }

    #[test]
    fn test_peer_request_before_hello_rejected() {
        let (init, mut resp) = setup();
        let req = init.build_peer_request();
        assert_eq!(resp.process(req), Err(ZcError::UnexpectedMessage));
    }

    #[test]
    fn test_closed_session_rejects_all() {
        let (mut init, mut resp) = setup();
        // Close immediately.
        let close = init.build_close();
        resp.process(close).unwrap();
        // Further messages should be rejected.
        let hello = ZcMessage::Hello {
            sender_id: [1u8; 32],
            nonce: [0u8; 32],
            timestamp: resp.current_time,
        };
        assert_eq!(resp.process(hello), Err(ZcError::SessionClosed));
    }

    #[test]
    fn test_nonce_cache_replay_detection() {
        let mut cache = NonceCache::new();
        let nonce = [0xDDu8; 32];
        assert!(!cache.is_replay(&nonce));
        cache.record(nonce);
        assert!(cache.is_replay(&nonce));
        cache.flush();
        assert!(!cache.is_replay(&nonce));
    }
}
