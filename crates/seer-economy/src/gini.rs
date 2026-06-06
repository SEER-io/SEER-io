//! # Seer Gini Coefficient & Inequality Modeling
//!
//! Seer utilises a dynamic reward scaling model that incorporates the Gini
//! coefficient to promote network health and decentralisation. This module
//! computes inequality metrics that influence how rewards are distributed
//! across participants.
//!
//! ## Gini Coefficient
//! The Gini coefficient G ∈ [0, 1] measures wealth inequality:
//! - G = 0 → perfect equality (all accounts hold the same balance)
//! - G = 1 → maximum inequality (one account holds all tokens)
//!
//! ## Reward Scaling
//! When G > TARGET_GINI, block rewards are boosted for smaller accounts to
//! encourage redistribution. When G < TARGET_GINI, rewards are slightly
//! reduced to prevent hyper-equalisation.

use crate::constants::{GINI_SCALE, TARGET_GINI_SCALED};

// ─── Fixed-point precision ────────────────────────────────────────────────────

/// Internal precision scale for Gini calculations (10^6 = one part per million).
pub const GINI_PRECISION: u64 = 1_000_000;

// ─── Gini computation ─────────────────────────────────────────────────────────

/// Computes the Gini coefficient over a slice of account balances.
///
/// Uses the standard sorted-list formula:
/// ```text
/// G = (2 * Σ(i * x_i) / (n * Σ x_i)) - (n + 1) / n
/// ```
/// where balances are sorted in ascending order and `i` is 1-based.
///
/// Returns the coefficient scaled by `GINI_PRECISION` (e.g., 350_000 = 0.35).
/// Returns 0 if the slice is empty or all balances are zero.
pub fn gini_coefficient(balances: &[u64]) -> u64 {
    let n = balances.len();
    if n == 0 {
        return 0;
    }

    let total: u64 = balances.iter().sum();
    if total == 0 {
        return 0;
    }

    // Sort ascending.
    let mut sorted = balances.to_vec();
    sorted.sort_unstable();

    // Σ (i * x_i), 1-based.
    let weighted_sum: u128 = sorted
        .iter()
        .enumerate()
        .map(|(i, &x)| (i as u128 + 1) * x as u128)
        .sum();

    let n128 = n as u128;
    let total128 = total as u128;
    let precision = GINI_PRECISION as u128;

    // G = (2 * weighted_sum) / (n * total) - (n + 1) / n
    // Scaled by GINI_PRECISION:
    // G_scaled = precision * (2 * weighted_sum - total * (n + 1)) / (n * total)
    let numerator = 2u128 * weighted_sum;
    let correction = total128 * (n128 + 1);

    if numerator < correction {
        // Numerical underflow guard (should not happen with valid inputs).
        return 0;
    }

    let g_scaled = precision * (numerator - correction) / (n128 * total128);
    g_scaled.min(GINI_PRECISION as u128) as u64
}

/// Converts a raw `gini_coefficient` result (scaled by `GINI_PRECISION`) to
/// the coarser `GINI_SCALE` representation used by `constants.rs`.
pub fn to_gini_scale(gini_precision: u64) -> u64 {
    gini_precision * GINI_SCALE / GINI_PRECISION
}

// ─── Reward multiplier ────────────────────────────────────────────────────────

/// Computes the Gini-scaled reward multiplier for a given account balance.
///
/// The multiplier is expressed as a fixed-point value scaled by
/// `REWARD_MULTIPLIER_SCALE`. A value of `REWARD_MULTIPLIER_SCALE` means
/// no adjustment (1×).
///
/// # Logic
/// - If the network Gini is above target, smaller accounts receive a bonus
///   (up to `MAX_BOOST`) and larger accounts receive a slight penalty.
/// - If the network Gini is at or below target, all accounts receive 1×.
///
/// `account_balance` and `mean_balance` are in base units.
/// `network_gini` is scaled by `GINI_PRECISION`.
pub const REWARD_MULTIPLIER_SCALE: u64 = 1_000;
pub const MAX_BOOST: u64 = 1_500; // 1.5× maximum boost
pub const MIN_MULTIPLIER: u64 = 750; // 0.75× minimum (for large accounts when G is high)

pub fn reward_multiplier(
    account_balance: u64,
    mean_balance: u64,
    network_gini: u64,
) -> u64 {
    let target_gini = TARGET_GINI_SCALED * GINI_PRECISION / GINI_SCALE;

    if network_gini <= target_gini || mean_balance == 0 {
        return REWARD_MULTIPLIER_SCALE; // 1× — no adjustment needed
    }

    // How far above target is the Gini? (0 to GINI_PRECISION)
    let excess = network_gini.saturating_sub(target_gini);
    // Normalise excess to [0, 1] relative to maximum possible deviation.
    let max_excess = GINI_PRECISION - target_gini;
    let excess_ratio = (excess * GINI_PRECISION)
        .checked_div(max_excess)
        .unwrap_or(GINI_PRECISION);

    // Accounts below mean get a boost; accounts above mean get a penalty.
    if account_balance <= mean_balance {
        // Boost: linearly interpolate from 1× to MAX_BOOST based on excess_ratio.
        let boost_range = MAX_BOOST - REWARD_MULTIPLIER_SCALE;
        REWARD_MULTIPLIER_SCALE + boost_range * excess_ratio / GINI_PRECISION
    } else {
        // Penalty: linearly interpolate from 1× down to MIN_MULTIPLIER.
        let penalty_range = REWARD_MULTIPLIER_SCALE - MIN_MULTIPLIER;
        REWARD_MULTIPLIER_SCALE - penalty_range * excess_ratio / GINI_PRECISION
    }
}

/// Applies the Gini-scaled reward multiplier to a base reward amount.
pub fn scale_reward(base_reward: u64, multiplier: u64) -> u64 {
    base_reward
        .saturating_mul(multiplier)
        .saturating_div(REWARD_MULTIPLIER_SCALE)
}

// ─── Sampling ─────────────────────────────────────────────────────────────────

/// Computes the Gini coefficient over a random sample of balances.
///
/// For large networks, computing Gini over all accounts is expensive. This
/// function accepts a pre-sampled slice (the caller is responsible for
/// sampling) and computes the coefficient over that subset.
///
/// The result is identical to `gini_coefficient` — sampling is the caller's
/// responsibility to keep this function pure and deterministic.
pub fn sampled_gini(sample: &[u64]) -> u64 {
    gini_coefficient(sample)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gini_perfect_equality() {
        let balances = vec![100u64; 10];
        let g = gini_coefficient(&balances);
        assert_eq!(g, 0, "perfect equality should give G = 0");
    }

    #[test]
    fn test_gini_maximum_inequality() {
        // One account holds everything.
        let mut balances = vec![0u64; 9];
        balances.push(1_000_000);
        let g = gini_coefficient(&balances);
        // G should be close to (n-1)/n = 0.9 for n=10.
        // Scaled: 900_000 ± small rounding.
        assert!(g > 850_000, "G should be close to 0.9, got {g}");
    }

    #[test]
    fn test_gini_empty_slice() {
        assert_eq!(gini_coefficient(&[]), 0);
    }

    #[test]
    fn test_gini_all_zero() {
        assert_eq!(gini_coefficient(&[0, 0, 0]), 0);
    }

    #[test]
    fn test_gini_two_equal() {
        let g = gini_coefficient(&[50, 50]);
        assert_eq!(g, 0);
    }

    #[test]
    fn test_gini_two_unequal() {
        // [0, 100]: G = 0.5
        let g = gini_coefficient(&[0, 100]);
        assert_eq!(g, GINI_PRECISION / 2);
    }

    #[test]
    fn test_gini_in_range() {
        let balances: Vec<u64> = (1..=100).collect();
        let g = gini_coefficient(&balances);
        assert!(g > 0 && g < GINI_PRECISION, "G should be in (0, 1), got {g}");
    }

    #[test]
    fn test_to_gini_scale() {
        // 0.35 in GINI_PRECISION = 350_000; should convert to 35 in GINI_SCALE.
        let g = 350_000u64;
        assert_eq!(to_gini_scale(g), 35);
    }

    #[test]
    fn test_reward_multiplier_no_adjustment_below_target() {
        // Gini at target → multiplier = 1×
        let target_gini = TARGET_GINI_SCALED * GINI_PRECISION / GINI_SCALE;
        let m = reward_multiplier(1_000, 1_000, target_gini);
        assert_eq!(m, REWARD_MULTIPLIER_SCALE);
    }

    #[test]
    fn test_reward_multiplier_small_account_gets_boost() {
        // Gini well above target, small account → multiplier > 1×
        let high_gini = 800_000u64; // 0.8
        let m = reward_multiplier(100, 10_000, high_gini);
        assert!(m > REWARD_MULTIPLIER_SCALE, "small account should get boost, got {m}");
    }

    #[test]
    fn test_reward_multiplier_large_account_gets_penalty() {
        // Gini well above target, large account → multiplier < 1×
        let high_gini = 800_000u64;
        let m = reward_multiplier(100_000, 1_000, high_gini);
        assert!(m < REWARD_MULTIPLIER_SCALE, "large account should get penalty, got {m}");
    }

    #[test]
    fn test_scale_reward() {
        let base = 1_000u64;
        let multiplier = 1_500u64; // 1.5×
        let scaled = scale_reward(base, multiplier);
        assert_eq!(scaled, 1_500);
    }
}
