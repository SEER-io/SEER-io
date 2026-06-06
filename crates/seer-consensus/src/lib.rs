//! # Seer Consensus Module
//!
//! ## Architectural Significance
//! The `seer-consensus` crate is the decision-making engine of the Seer network. It orchestrates
//! the multi-layered consensus protocols required to maintain agreement across decentralized
//! nodes. By combining Proof-of-Work (PoW) for Sybil resistance with Fishermen auditing and
//! vertical patching for self-healing, it ensures high availability and Byzantine fault tolerance.
//!
//! ## Technical Specifications
//! - **Sybil Resistance**: Implements PoW mechanisms for fair block production.
//! - **Auditing**: Fishermen nodes provide continuous verification of state transitions.
//! - **Self-Healing**: Vertical patches allow for rapid recovery from invalid states or forks.
//!
//! ## Invariants
//! - Consensus rules must be applied deterministically across all network nodes.
//! - Any state correction via vertical patching must be backed by majority-verified fraud proofs.

pub mod fishermen;
pub mod pow;
pub mod vertical_patch;
