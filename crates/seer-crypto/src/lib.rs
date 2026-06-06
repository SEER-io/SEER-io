//! # Seer Cryptography Module
//!
//! ## Overview
//! This module provides the core cryptographic tools required for the Seer network. It handles 
//! identity generation, secure channel establishment, and data integrity verification.
//!
//! ## Architectural Role
//! The `seer-crypto` crate underpins the security of the ADNL transport layer. It ensures 
//! that all node identities are abstract and verifiable, and that initial peer 
//! discovery is performed securely through zero-channel bootstrapping.
//!
//! ## Technical Specs
//! - **ADNL Identities**: 256-bit abstract identities hashed from public keys (SHA-256).
//! - **Zero-Channel**: Bootstrapping protocol for encrypted datagram tunnels.
//! - **Hashing**: Standardized SHA-256 implementation for BoC and PoW.
//!
//! ## Invariants
//! - Cryptographic operations must be side-channel resistant.
//! - All identity-related hashes must be collision-resistant and 256-bit minimum.

pub mod adnl;
pub mod zero_channel;
