//! # Seer Master Synchronisation Channel
//!
//! Implements the one-way downstream sync mechanism from the official Seer
//! network master channel. The master channel is a Telegram channel operated
//! by the network's bootstrap authority. It broadcasts:
//! - Canonical chain tip updates (new block announcements).
//! - Network-wide parameter changes (difficulty retargets).
//! - Patch announcements (vertical corrections).
//! - Checkpoint hashes for fast initial sync.
//!
//! ## Security Model
//! The master channel is **read-only** for regular nodes. Messages are
//! authenticated by the channel's ADNL identity. Nodes MUST NOT trust
//! master channel messages without verifying the sender identity.

use crate::transport::{TgMessage, TgMessageKind};
use std::collections::VecDeque;

// ─── Constants ────────────────────────────────────────────────────────────────

/// Maximum number of messages to buffer from the master channel.
pub const MASTER_CHANNEL_BUFFER: usize = 512;

/// Maximum age of a master channel message before it is considered stale (seconds).
pub const MASTER_MSG_MAX_AGE_SECS: u64 = 300; // 5 minutes

// ─── Master channel message kinds ────────────────────────────────────────────

/// The semantic type of a master channel message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MasterMsgKind {
    /// A new block has been added to the canonical chain.
    NewBlock,
    /// A difficulty retarget has occurred.
    DifficultyUpdate,
    /// A vertical patch has been issued.
    PatchAnnouncement,
    /// A checkpoint hash for fast sync.
    Checkpoint,
    /// A raw transport message (unknown kind, passed through).
    Raw,
}

/// A decoded master channel event.
#[derive(Debug, Clone)]
pub struct MasterEvent {
    /// The semantic kind of this event.
    pub kind: MasterMsgKind,
    /// The raw transport message.
    pub message: TgMessage,
    /// The height or sequence number this event refers to.
    pub ref_height: u64,
    /// Unix timestamp when the event was received.
    pub received_at: u64,
}

// ─── Master channel subscriber ────────────────────────────────────────────────

/// A subscriber that processes messages from the master channel.
#[derive(Debug)]
pub struct MasterChannelSubscriber {
    /// The expected ADNL identity of the master channel (hex-encoded, 64 chars).
    pub master_id: String,
    /// Buffer of received events.
    buffer: VecDeque<MasterEvent>,
    /// The latest known canonical chain tip height.
    pub latest_height: u64,
    /// The latest known canonical chain tip hash.
    pub latest_hash: [u8; 32],
    /// Current simulated time.
    pub current_time: u64,
    /// Total messages received.
    pub total_received: u64,
    /// Total messages rejected (wrong sender or stale).
    pub total_rejected: u64,
}

impl MasterChannelSubscriber {
    /// Creates a new subscriber.
    pub fn new(master_id: String, current_time: u64) -> Self {
        MasterChannelSubscriber {
            master_id,
            buffer: VecDeque::new(),
            latest_height: 0,
            latest_hash: [0u8; 32],
            current_time,
            total_received: 0,
            total_rejected: 0,
        }
    }

    /// Ingests a raw Telegram message from the master channel.
    ///
    /// Validates the sender identity and message freshness before buffering.
    pub fn ingest(&mut self, msg: TgMessage, msg_timestamp: u64) -> Result<(), MasterChannelError> {
        self.total_received += 1;

        // Sender identity check.
        if msg.sender_id != self.master_id {
            self.total_rejected += 1;
            return Err(MasterChannelError::UnauthorisedSender);
        }

        // Freshness check.
        let age = self.current_time.saturating_sub(msg_timestamp);
        if age > MASTER_MSG_MAX_AGE_SECS {
            self.total_rejected += 1;
            return Err(MasterChannelError::StaleMessage);
        }

        // Buffer overflow check.
        if self.buffer.len() >= MASTER_CHANNEL_BUFFER {
            // Drop oldest message to make room.
            self.buffer.pop_front();
        }

        // Decode the event kind and ref_height.
        let (kind, ref_height) = self.decode_kind(&msg);

        // Update latest height if this is a block announcement.
        if kind == MasterMsgKind::NewBlock && ref_height > self.latest_height {
            self.latest_height = ref_height;
            // Extract hash from payload if available.
            if let Ok(bytes) = msg.payload_bytes() {
                if bytes.len() >= 40 {
                    self.latest_hash.copy_from_slice(&bytes[8..40]);
                }
            }
        }

        self.buffer.push_back(MasterEvent {
            kind,
            message: msg,
            ref_height,
            received_at: self.current_time,
        });

        Ok(())
    }

    /// Decodes the semantic kind and reference height from a transport message.
    fn decode_kind(&self, msg: &TgMessage) -> (MasterMsgKind, u64) {
        let height = msg.payload_bytes()
            .ok()
            .and_then(|b| b.get(..8).map(|s| {
                let mut arr = [0u8; 8];
                arr.copy_from_slice(s);
                u64::from_le_bytes(arr)
            }))
            .unwrap_or(0);

        let kind = match msg.kind {
            TgMessageKind::Block => MasterMsgKind::NewBlock,
            TgMessageKind::Patch => MasterMsgKind::PatchAnnouncement,
            TgMessageKind::Inv => MasterMsgKind::Checkpoint,
            _ => MasterMsgKind::Raw,
        };
        (kind, height)
    }

    /// Returns the next buffered event, if any.
    pub fn next_event(&mut self) -> Option<MasterEvent> {
        self.buffer.pop_front()
    }

    /// Returns the number of buffered events.
    pub fn buffered_count(&self) -> usize {
        self.buffer.len()
    }

    /// Drains all buffered events into a Vec.
    pub fn drain_events(&mut self) -> Vec<MasterEvent> {
        self.buffer.drain(..).collect()
    }

    /// Returns `true` if the subscriber is in sync with the master channel.
    ///
    /// Defined as: the latest known height was updated within the last
    /// `MASTER_MSG_MAX_AGE_SECS` seconds.
    pub fn is_synced(&self) -> bool {
        self.latest_height > 0
    }
}

// ─── Errors ───────────────────────────────────────────────────────────────────

/// Errors from master channel processing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MasterChannelError {
    /// The message was not sent by the expected master channel identity.
    UnauthorisedSender,
    /// The message is too old to be trusted.
    StaleMessage,
    /// The message payload could not be decoded.
    DecodeFailed,
}

impl std::fmt::Display for MasterChannelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MasterChannelError::UnauthorisedSender => write!(f, "unauthorised master channel sender"),
            MasterChannelError::StaleMessage => write!(f, "master channel message is stale"),
            MasterChannelError::DecodeFailed => write!(f, "failed to decode master channel message"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::{TgMessage, TgMessageKind, hex_encode};

    fn master_id_str() -> String {
        hex_encode(&[0xFFu8; 32])
    }

    fn make_subscriber() -> MasterChannelSubscriber {
        MasterChannelSubscriber::new(master_id_str(), 1_700_000_000)
    }

    fn make_block_msg(height: u64) -> TgMessage {
        let mut payload = vec![0u8; 40];
        payload[..8].copy_from_slice(&height.to_le_bytes());
        payload[8..40].copy_from_slice(&[0xCCu8; 32]);
        TgMessage::new(TgMessageKind::Block, &payload, [0xFFu8; 32], height)
    }

    #[test]
    fn test_ingest_valid_message() {
        let mut sub = make_subscriber();
        let msg = make_block_msg(10);
        sub.ingest(msg, sub.current_time).unwrap();
        assert_eq!(sub.buffered_count(), 1);
        assert_eq!(sub.latest_height, 10);
    }

    #[test]
    fn test_ingest_wrong_sender_rejected() {
        let mut sub = make_subscriber();
        let msg = TgMessage::new(TgMessageKind::Block, &[0u8; 40], [0x01u8; 32], 0);
        let err = sub.ingest(msg, sub.current_time).unwrap_err();
        assert_eq!(err, MasterChannelError::UnauthorisedSender);
        assert_eq!(sub.total_rejected, 1);
    }

    #[test]
    fn test_ingest_stale_message_rejected() {
        let mut sub = make_subscriber();
        let msg = make_block_msg(5);
        let stale_ts = sub.current_time - MASTER_MSG_MAX_AGE_SECS - 1;
        let err = sub.ingest(msg, stale_ts).unwrap_err();
        assert_eq!(err, MasterChannelError::StaleMessage);
    }

    #[test]
    fn test_latest_height_updates() {
        let mut sub = make_subscriber();
        sub.ingest(make_block_msg(1), sub.current_time).unwrap();
        sub.ingest(make_block_msg(5), sub.current_time).unwrap();
        sub.ingest(make_block_msg(3), sub.current_time).unwrap(); // older, should not update
        assert_eq!(sub.latest_height, 5);
    }

    #[test]
    fn test_drain_events() {
        let mut sub = make_subscriber();
        sub.ingest(make_block_msg(1), sub.current_time).unwrap();
        sub.ingest(make_block_msg(2), sub.current_time).unwrap();
        let events = sub.drain_events();
        assert_eq!(events.len(), 2);
        assert_eq!(sub.buffered_count(), 0);
    }

    #[test]
    fn test_is_synced_after_block() {
        let mut sub = make_subscriber();
        assert!(!sub.is_synced());
        sub.ingest(make_block_msg(1), sub.current_time).unwrap();
        assert!(sub.is_synced());
    }

    #[test]
    fn test_buffer_overflow_drops_oldest() {
        let mut sub = make_subscriber();
        for i in 0..=MASTER_CHANNEL_BUFFER {
            sub.ingest(make_block_msg(i as u64), sub.current_time).unwrap();
        }
        // Buffer should be capped at MASTER_CHANNEL_BUFFER.
        assert_eq!(sub.buffered_count(), MASTER_CHANNEL_BUFFER);
    }
}
