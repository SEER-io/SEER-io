//! # Seer Core Module
//!
//! ## Overview
//! The `seer-core` crate is the primary repository for blockchain primitives within the Seer ecosystem.
//! It implements the fundamental data structures and state transition logic required for a 
//! TON-inspired, decentralized PoW system.
//!
//! ## Architectural Role
//! Serving as the foundational backbone, this crate exposes core primitives for serialization, 
//! cell management, and block processing. It enables the high-performance sharding and 
//! asynchronous messaging characteristic of the Seer architecture.
//!
//! ## Technical Specs
//! - **Cell-based State**: Implements the Bag-of-Cells (BoC) model (128-byte data, 4 refs).
//! - **Block Schemas**: Defines structures for horizontal blocks and self-healing Vertical Patches.
//! - **Atomic Transactions**: Handles cryptographic verification and Merkle-proof generation.
//!
//! ## Invariants
//! - All state transitions must result in a valid Merkle root over the modified cells.
//! - Block height must strictly increase monotonically within a shard.

pub mod block;
pub mod cell;
pub mod ledger;
pub mod transaction;
