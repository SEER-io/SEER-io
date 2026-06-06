//! # Seer Economic Constants
//!
//! Houses the fundamental mathematical constants that drive the Seer network's
//! economy. These values are fixed or derived from genesis to ensure that every
//! node in the network follows the same deterministic monetary path.
//!
//! All values are sourced from `config/genesis.toml` and must never change
//! post-genesis without a network-wide consensus upgrade.

// ─── Supply parameters ────────────────────────────────────────────────────────

/// Initial token supply at genesis (100 million base units).
pub const INITIAL_SUPPLY: u64 = 100_000_000;

/// Long-term target maximum supply (1 billion base units).
pub const FINAL_SUPPLY_TARGET: u64 = 1_000_000_000;

/// The smallest indivisible unit of the Seer token (analogous to satoshis).
/// 1 SEER = 10^8 base units.
pub const BASE_UNIT_PRECISION: u64 = 100_000_000;

/// Maximum supply that can ever exist (hard cap = final supply target).
pub const MAX_SUPPLY: u64 = FINAL_SUPPLY_TARGET;

// ─── Block reward schedule ────────────────────────────────────────────────────

/// Base block reward at genesis, in base units (50 SEER × precision).
pub const BLOCK_REWARD_BASE: u64 = 50 * BASE_UNIT_PRECISION;

/// Number of blocks between reward halvings.
/// At 10 s/block this gives a halving every ~4 years (≈ 12 614 400 blocks).
pub const HALVING_INTERVAL: u64 = 12_614_400;

/// Minimum block reward after all halvings (dust floor).
pub const MIN_BLOCK_REWARD: u64 = BASE_UNIT_PRECISION / 100; // 0.01 SEER

// ─── Burn & fee parameters ────────────────────────────────────────────────────

/// Base transaction burn rate as a fixed-point fraction (numerator / RATE_DENOM).
/// 0.01 → 1% of each transaction fee is burned.
pub const BURN_RATE_BASE_NUM: u64 = 1;
pub const RATE_DENOM: u64 = 100;

/// Maximum burn rate cap (10%).
pub const BURN_RATE_MAX_NUM: u64 = 10;

/// Minimum transaction fee in base units.
pub const MIN_TX_FEE: u64 = BASE_UNIT_PRECISION / 1_000; // 0.001 SEER

// ─── Economic health targets ──────────────────────────────────────────────────

/// Target Gini coefficient for the network wealth distribution.
/// Expressed as a fixed-point integer scaled by GINI_SCALE (0.35 → 35).
pub const TARGET_GINI_SCALED: u64 = 35;
pub const GINI_SCALE: u64 = 100;

/// Target token velocity (average number of times a token changes hands per
/// epoch). Scaled by VELOCITY_SCALE.
pub const VELOCITY_TARGET_SCALED: u64 = 500; // 5.0 × 100
pub const VELOCITY_SCALE: u64 = 100;

/// Sentiment buffer: the fraction of the velocity target used as a dead-band
/// before adjustments are triggered. Scaled by SENTIMENT_SCALE.
pub const SENTIMENT_BUFFER_SCALED: u64 = 15; // 0.15 × 100
pub const SENTIMENT_SCALE: u64 = 100;

// ─── Timing constants ─────────────────────────────────────────────────────────

/// Target block time in seconds.
pub const TARGET_BLOCK_TIME_SECS: u64 = 10;

/// Number of blocks per day (at target block time).
pub const BLOCKS_PER_DAY: u64 = 86_400 / TARGET_BLOCK_TIME_SECS;

/// Number of blocks per year (approximate).
pub const BLOCKS_PER_YEAR: u64 = BLOCKS_PER_DAY * 365;

// ─── Reward calculation helpers ───────────────────────────────────────────────

/// Computes the block reward at a given block height, applying halving logic.
///
/// The reward halves every `HALVING_INTERVAL` blocks and is floored at
/// `MIN_BLOCK_REWARD`.
pub fn block_reward_at(height: u64) -> u64 {
    let halvings = height / HALVING_INTERVAL;
    if halvings >= 64 {
        return MIN_BLOCK_REWARD;
    }
    let reward = BLOCK_REWARD_BASE >> halvings;
    reward.max(MIN_BLOCK_REWARD)
}

/// Computes the burn amount for a given fee, using the base burn rate.
///
/// Returns `fee * BURN_RATE_BASE_NUM / RATE_DENOM`, saturating at the fee itself.
pub fn burn_amount(fee: u64) -> u64 {
    fee.saturating_mul(BURN_RATE_BASE_NUM) / RATE_DENOM
}

/// Computes the burn amount for a given fee using a dynamic burn rate numerator.
///
/// `rate_num` is the numerator over `RATE_DENOM`. Clamped to `BURN_RATE_MAX_NUM`.
pub fn dynamic_burn_amount(fee: u64, rate_num: u64) -> u64 {
    let clamped = rate_num.min(BURN_RATE_MAX_NUM);
    fee.saturating_mul(clamped) / RATE_DENOM
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_supply_less_than_max() {
        assert!(INITIAL_SUPPLY < MAX_SUPPLY);
    }

    #[test]
    fn test_block_reward_genesis() {
        assert_eq!(block_reward_at(0), BLOCK_REWARD_BASE);
    }

    #[test]
    fn test_block_reward_first_halving() {
        assert_eq!(block_reward_at(HALVING_INTERVAL), BLOCK_REWARD_BASE / 2);
    }

    #[test]
    fn test_block_reward_second_halving() {
        assert_eq!(block_reward_at(HALVING_INTERVAL * 2), BLOCK_REWARD_BASE / 4);
    }

    #[test]
    fn test_block_reward_floor() {
        // After 64 halvings the reward should be floored at MIN_BLOCK_REWARD.
        assert_eq!(block_reward_at(HALVING_INTERVAL * 64), MIN_BLOCK_REWARD);
    }

    #[test]
    fn test_burn_amount_base_rate() {
        // 1% of 10_000 = 100
        assert_eq!(burn_amount(10_000), 100);
    }

    #[test]
    fn test_dynamic_burn_capped() {
        // Rate 50% should be clamped to 10%.
        let fee = 1_000;
        assert_eq!(dynamic_burn_amount(fee, 50), dynamic_burn_amount(fee, BURN_RATE_MAX_NUM));
    }

    #[test]
    fn test_blocks_per_day() {
        assert_eq!(BLOCKS_PER_DAY, 8_640);
    }

    #[test]
    fn test_gini_scale_consistency() {
        // Target Gini of 0.35 should be representable within [0, GINI_SCALE].
        assert!(TARGET_GINI_SCALED <= GINI_SCALE);
    }
}
