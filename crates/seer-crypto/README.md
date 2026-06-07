# SEER Cryptographic Primitives

Provides the essential security layer for the network, focusing on identity management and message signing.

## Components

### 1. `adnl.rs`
Implements the Abstract Datagram Network Layer (ADNL) identity standard.
- **Node ID:** Derived as `SHA256(Ed25519_PublicKey)`.
- **Identity:** Ensures every bot node is a unique, cryptographically identifiable entity.

### 2. `zero_channel.rs`
Handles the logic for bootstrapping secure connections between peers without pre-existing trust.

## Key Management
- **Type:** Ed25519 (Edwards-curve Digital Signature Algorithm).
- **Utility:** Used for both Node Identity and Wallet Signatures (1:1 mapping).

---
*Identity is the Master Key of the SEER Network.*
