//! # Seer Economy Module
//!
//! ## Architectural Significance
//! The `seer-economy` crate is responsible for maintaining the financial equilibrium of the Seer
//! network. It consolidates economic mathematical models and vectors used to validate block
//! rewards, transaction fees, and monetary policy enforcement during state transitions.
//!
//! ## Technical Specifications
//! - **Monetary Policy**: Implements deterministic evolution of the asset supply.
//! - **Reward Scaling**: Dynamically adjusts block rewards based on network health and inequality.
//! - **Burn Mechanisms**: Calculates asset burn rates to offset inflation and maintain value.
//!
//! ## Invariants
//! - Economic calculations must be deterministic and overflow-safe.
//! - Total asset supply must strictly follow the curves defined in the genesis configuration.

pub mod constants;
pub mod gini;
pub mod metrics;
