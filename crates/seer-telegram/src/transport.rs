//! # Seer Telegram Transport Layer
//!
//! Serialises and deserialises network primitives into Telegram message
//! variants. The transport layer bridges the Seer P2P protocol with the
//! Telegram Bot API, encoding blockchain objects as structured text messages
//! that bots can relay through Telegram channels and groups.
//!
//! ## Message Variants
//! | Prefix | Payload          | Description                              |
//! |--------|------------------|------------------------------------------|
//! | `TX`   | hex-encoded tx   | Broadcast a pending transaction          |
//! | `BLOCK`| hex-encoded block| Announce a newly mined block             |
//! | `PATCH`| hex-encoded patch| Propagate a vertical correction patch    |
//! | `INV`  | hash list        | Inventory announcement (hashes only)     |
//! | `PING` | node id          | Liveness heartbeat                       |
//! | `PONG` | node id          | Heartbeat response                       |

// ─── Message variants ─────────────────────────────────────────────────────────

/// The type of a Telegram transport message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TgMessageKind {
    /// A pending transaction broadcast.
    Tx,
    /// A newly mined block announcement.
    Block,
    /// A vertical correction patch.
    Patch,
    /// An inventory announcement (list of hashes).
    Inv,
    /// A liveness ping.
    Ping,
    /// A liveness pong.
    Pong,
    /// Request a block by hash.
    GetBlock,
    /// Node health and chain tip broadcast.
    Status,
    /// Node registration via factory bot.
    Register,
    /// Peer discovery advertisement.
    Peer,
}

impl TgMessageKind {
    /// Returns the wire prefix string for this message kind.
    pub fn prefix(&self) -> &'static str {
        match self {
            TgMessageKind::Tx => "TX",
            TgMessageKind::Block => "BLOCK",
            TgMessageKind::Patch => "PATCH",
            TgMessageKind::Inv => "INV",
            TgMessageKind::Ping => "PING",
            TgMessageKind::Pong => "PONG",
            TgMessageKind::GetBlock => "GETBLOCK",
            TgMessageKind::Status => "STATUS",
            TgMessageKind::Register => "REGISTER",
            TgMessageKind::Peer => "PEER",
        }
    }

    /// Parses a message kind from its wire prefix.
    pub fn from_prefix(prefix: &str) -> Option<Self> {
        match prefix {
            "TX" => Some(TgMessageKind::Tx),
            "BLOCK" => Some(TgMessageKind::Block),
            "PATCH" => Some(TgMessageKind::Patch),
            "INV" => Some(TgMessageKind::Inv),
            "PING" => Some(TgMessageKind::Ping),
            "PONG" => Some(TgMessageKind::Pong),
            "GETBLOCK" => Some(TgMessageKind::GetBlock),
            "STATUS" => Some(TgMessageKind::Status),
            "REGISTER" => Some(TgMessageKind::Register),
            "PEER" => Some(TgMessageKind::Peer),
            _ => None,
        }
    }
}

// ─── Transport message ────────────────────────────────────────────────────────

/// A Telegram transport message: a typed envelope around a hex-encoded payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TgMessage {
    /// The message kind (determines how the payload is interpreted).
    pub kind: TgMessageKind,
    /// Hex-encoded payload bytes.
    pub payload_hex: String,
    /// The ADNL identity of the sender (hex-encoded, 64 chars).
    pub sender_id: String,
    /// Sequence number for ordering messages from the same sender.
    pub seq: u64,
}

impl TgMessage {
    /// Creates a new transport message.
    pub fn new(kind: TgMessageKind, payload: &[u8], sender_id: [u8; 32], seq: u64) -> Self {
        TgMessage {
            kind,
            payload_hex: hex_encode(payload),
            sender_id: hex_encode(&sender_id),
            seq,
        }
    }

    /// Serialises the message to a Telegram-sendable text string.
    ///
    /// Format: `SEER/{KIND}/{SEQ}/{SENDER_ID}/{PAYLOAD_HEX}`
    pub fn to_wire(&self) -> String {
        format!(
            "SEER/{}/{}/{}/{}",
            self.kind.prefix(),
            self.seq,
            self.sender_id,
            self.payload_hex,
        )
    }

    /// Parses a wire-format string back into a `TgMessage`.
    pub fn from_wire(wire: &str) -> Result<Self, TransportError> {
        let parts: Vec<&str> = wire.splitn(5, '/').collect();
        if parts.len() != 5 {
            return Err(TransportError::MalformedMessage);
        }
        if parts[0] != "SEER" {
            return Err(TransportError::UnknownProtocol);
        }
        let kind = TgMessageKind::from_prefix(parts[1])
            .ok_or(TransportError::UnknownMessageKind)?;
        let seq: u64 = parts[2].parse().map_err(|_| TransportError::MalformedMessage)?;
        let sender_id = parts[3].to_string();
        if sender_id.len() != 64 {
            return Err(TransportError::MalformedMessage);
        }
        let payload_hex = parts[4].to_string();
        Ok(TgMessage { kind, payload_hex, sender_id, seq })
    }

    /// Decodes the payload from hex.
    pub fn payload_bytes(&self) -> Result<Vec<u8>, TransportError> {
        hex_decode(&self.payload_hex).map_err(|_| TransportError::InvalidHex)
    }
}

// ─── Transport errors ─────────────────────────────────────────────────────────

/// Errors from transport encoding/decoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    /// The message does not follow the expected wire format.
    MalformedMessage,
    /// The protocol prefix is not "SEER".
    UnknownProtocol,
    /// The message kind prefix is not recognised.
    UnknownMessageKind,
    /// Hex decoding failed.
    InvalidHex,
    /// Payload is empty.
    EmptyPayload,
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportError::MalformedMessage => write!(f, "malformed transport message"),
            TransportError::UnknownProtocol => write!(f, "unknown protocol prefix"),
            TransportError::UnknownMessageKind => write!(f, "unknown message kind"),
            TransportError::InvalidHex => write!(f, "invalid hex encoding"),
            TransportError::EmptyPayload => write!(f, "empty payload"),
        }
    }
}

// ─── Hex helpers ──────────────────────────────────────────────────────────────

/// Encodes bytes as a lowercase hex string.
pub fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Decodes a hex string into bytes.
pub fn hex_decode(hex: &str) -> Result<Vec<u8>, ()> {
    if hex.len() % 2 != 0 {
        return Err(());
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i+2], 16).map_err(|_| ()))
        .collect()
}

// ─── Codec helpers ────────────────────────────────────────────────────────────

/// Encodes a 32-byte hash as a TX inventory message.
pub fn encode_inv(hashes: &[[u8; 32]], sender_id: [u8; 32], seq: u64) -> TgMessage {
    let payload: Vec<u8> = hashes.iter().flat_map(|h| h.iter().copied()).collect();
    TgMessage::new(TgMessageKind::Inv, &payload, sender_id, seq)
}

/// Decodes an INV message payload into a list of 32-byte hashes.
pub fn decode_inv(msg: &TgMessage) -> Result<Vec<[u8; 32]>, TransportError> {
    let bytes = msg.payload_bytes()?;
    if bytes.len() % 32 != 0 {
        return Err(TransportError::MalformedMessage);
    }
    Ok(bytes
        .chunks(32)
        .map(|chunk| {
            let mut h = [0u8; 32];
            h.copy_from_slice(chunk);
            h
        })
        .collect())
}

/// Encodes a PING message.
pub fn encode_ping(sender_id: [u8; 32], seq: u64) -> TgMessage {
    TgMessage::new(TgMessageKind::Ping, &sender_id, sender_id, seq)
}

/// Encodes a PONG response to a PING.
pub fn encode_pong(sender_id: [u8; 32], seq: u64) -> TgMessage {
    TgMessage::new(TgMessageKind::Pong, &sender_id, sender_id, seq)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round_trip_tx_message() {
        let payload = b"fake_transaction_bytes_here";
        let sender = [0x01u8; 32];
        let msg = TgMessage::new(TgMessageKind::Tx, payload, sender, 42);
        let wire = msg.to_wire();
        assert!(wire.starts_with("SEER/TX/42/"));
        let parsed = TgMessage::from_wire(&wire).unwrap();
        assert_eq!(parsed.kind, TgMessageKind::Tx);
        assert_eq!(parsed.seq, 42);
        assert_eq!(parsed.payload_bytes().unwrap(), payload);
    }

    #[test]
    fn test_round_trip_block_message() {
        let payload = vec![0xBBu8; 64];
        let sender = [0x02u8; 32];
        let msg = TgMessage::new(TgMessageKind::Block, &payload, sender, 1);
        let wire = msg.to_wire();
        let parsed = TgMessage::from_wire(&wire).unwrap();
        assert_eq!(parsed.kind, TgMessageKind::Block);
        assert_eq!(parsed.payload_bytes().unwrap(), payload);
    }

    #[test]
    fn test_malformed_wire_rejected() {
        assert_eq!(TgMessage::from_wire("garbage"), Err(TransportError::MalformedMessage));
        assert_eq!(TgMessage::from_wire("NOTSR/TX/0/aabb/cc"), Err(TransportError::UnknownProtocol));
        assert_eq!(TgMessage::from_wire("SEER/UNKNOWN/0/aabb/cc"), Err(TransportError::UnknownMessageKind));
    }

    #[test]
    fn test_inv_encode_decode() {
        let hashes = vec![[0x01u8; 32], [0x02u8; 32], [0x03u8; 32]];
        let sender = [0x05u8; 32];
        let msg = encode_inv(&hashes, sender, 7);
        let decoded = decode_inv(&msg).unwrap();
        assert_eq!(decoded, hashes);
    }

    #[test]
    fn test_ping_pong() {
        let sender = [0x0Au8; 32];
        let ping = encode_ping(sender, 1);
        assert_eq!(ping.kind, TgMessageKind::Ping);
        let pong = encode_pong(sender, 1);
        assert_eq!(pong.kind, TgMessageKind::Pong);
    }

    #[test]
    fn test_hex_encode_decode_roundtrip() {
        let bytes = vec![0x00u8, 0xAB, 0xFF, 0x12];
        let hex = hex_encode(&bytes);
        assert_eq!(hex, "00abff12");
        assert_eq!(hex_decode(&hex).unwrap(), bytes);
    }

    #[test]
    fn test_hex_decode_odd_length_fails() {
        assert!(hex_decode("abc").is_err());
    }

    #[test]
    fn test_all_message_kinds_roundtrip() {
        let kinds = [
            TgMessageKind::Tx, TgMessageKind::Block, TgMessageKind::Patch,
            TgMessageKind::Inv, TgMessageKind::Ping, TgMessageKind::Pong,
        ];
        for kind in &kinds {
            let parsed = TgMessageKind::from_prefix(kind.prefix()).unwrap();
            assert_eq!(&parsed, kind);
        }
    }
}
