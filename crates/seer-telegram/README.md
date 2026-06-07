# SEER Telegram Transport Layer

This crate turns the Telegram API into a robust transport layer for a decentralized blockchain network.

## Components

### 1. `bot_node.rs`
The primary logic daemon. Handles state management for individual bot instances and routes commands to the underlying network logic.

### 2. `factory.rs`
Automates the creation and registration of new nodes. It "spins up" a node by generating an identity and linking it to a Telegram bot.

### 3. `master_channel.rs`
Implements the "Channel as a Global CLI" pattern. This allows bots to broadcast significant events (like mining successes or new node births) to a central administrative channel for network-wide observability.

---
*The Chat Window is the New Terminal.*
