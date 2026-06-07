# SEER Consensus Crate

This crate implements the Proof-of-Work (PoW) and fork-choice rules that allow decentralized nodes to agree on the history of the SEER blockchain.

## Components

### 1. `pow.rs`
The heart of the mining engine.
- **Algorithm:** Double SHA-256 (SHA256d), consistent with the original Bitcoin specification but adapted for serverless execution.
- **Difficulty:** Implements the bit-target mechanism and the `retarget` algorithm that adjusts mining difficulty based on network hashrate.

### 2. `verification.rs` (Planned)
Formalizes the verification of fraud proofs (Fishermen) and provides the logic for "slashing" invalid block submissions.

## Principles
- **Fairness:** 10-second block time target ensures rapid finality and accessibility for small miners.
- **Efficiency:** Optimized hashing buffers to minimize CPU usage in edge environments (Cloudflare/Termux).

---
*SEER: Securing the Edge.*
