# SEER Core Crate

The `seer-core` crate defines the fundamental data structures and state transition logic for the SEER network. It is designed to be the "source of truth" for what constitutes a valid block, transaction, and ledger state.

## Components

### 1. `block.rs`
Defines the `Block` and `BlockHeader` structures.
- **Protocol:** Strict 92-byte binary headers for hashing.
- **Validation:** Implements standard linkage checks (prev_hash, height sequence).

### 2. `transaction.rs`
Handles the SEER transaction model.
- **Structure:** `sender`, `recipient`, `amount`, `fee`, and `signature`.
- **Ordering:** Logic for fee-based mempool prioritization.

### 3. `ledger.rs`
The global state machine.
- **Account State:** Tracks balances and nonces for every wallet on the network.
- **Atomic Updates:** Processes entire blocks of transactions at once, ensuring balance consistency.

---
*Part of the SEER Network Protocol.*
