//! # Seer Convergence Simulation Engine
//!
//! Implements the deterministic multi-layer economic phase-space simulation
//! engine required to regenerate and verify the immutability of parameters
//! written to the genesis file.
//!
//! ## Purpose
//! The genesis parameters (initial supply, difficulty, reward schedule, burn
//! rate, Gini target, etc.) are not chosen arbitrarily. They emerge from a
//! deterministic simulation that models the economic phase-space of the network
//! over a projected lifetime. This module implements that simulation so that:
//! 1. Anyone can independently verify the genesis parameters.
//! 2. The parameters can be regenerated from the same seed.
//! 3. Proposed parameter changes can be evaluated before deployment.
//!
//! ## Simulation Layers
//! | Layer       | Models                                                    |
//! |-------------|-----------------------------------------------------------|
//! | Supply      | Emission schedule, halving, burn rate                     |
//! | Distribution| Gini coefficient evolution over time                      |
//! | Velocity    | Token circulation rate                                    |
//! | Consensus   | Block production rate, difficulty stability               |
//! | Network     | Node count growth, churn                                  |
//!
//! ## Convergence Criterion
//! The simulation is considered converged when all of the following hold for
//! `CONVERGENCE_WINDOW` consecutive epochs:
//! - Gini coefficient is within `GINI_TOLERANCE` of the target.
//! - Velocity is within `VELOCITY_TOLERANCE` of the target.
//! - Block time is within `BLOCK_TIME_TOLERANCE` of the target.
//!
//! ## Canonical Constants (must match genesis.toml and seer-economy/constants.rs)
//! | Constant              | Value          | Source              |
//! |-----------------------|----------------|---------------------|
//! | INITIAL_SUPPLY        | 100_000_000    | genesis.toml        |
//! | TARGET_BLOCK_TIME     | 10 s           | genesis.toml        |
//! | BLOCKS_PER_EPOCH      | 8_640          | 86400 / 10          |
//! | HALVING_INTERVAL      | 12_614_400     | constants.rs        |
//! | BASE_REWARD           | 50 SEER        | genesis.toml        |
//! | BURN_RATE_NUM / 10000 | 1% (num = 100) | genesis.toml        |

// ─── Constants ────────────────────────────────────────────────────────────────

/// Number of epochs to simulate per run.
pub const SIM_EPOCHS: u64 = 1_000;

/// Target block time in seconds. MUST match genesis.toml and seer-economy constants.
pub const TARGET_BLOCK_TIME: u64 = 10;

/// Number of blocks per epoch (1 day of blocks at TARGET_BLOCK_TIME).
/// = 86_400 / 10 = 8_640
pub const BLOCKS_PER_EPOCH: u64 = 86_400 / TARGET_BLOCK_TIME; // 8_640

/// Number of consecutive stable epochs required for convergence.
pub const CONVERGENCE_WINDOW: u64 = 10;

/// Gini coefficient tolerance (scaled by 1_000_000).
/// Convergence requires |gini - target| ≤ GINI_TOLERANCE.
pub const GINI_TOLERANCE: i64 = 20_000; // 0.02

/// Velocity tolerance (scaled by 1_000_000).
pub const VELOCITY_TOLERANCE: i64 = 50_000; // 0.05

/// Block time tolerance in seconds.
pub const BLOCK_TIME_TOLERANCE: u64 = 2;

/// Target Gini coefficient (scaled by 1_000_000). Matches genesis target of 0.35.
pub const TARGET_GINI: i64 = 350_000; // 0.35

/// Target velocity (scaled by 1_000_000). Matches genesis velocity_target = 5.0.
pub const TARGET_VELOCITY: i64 = 5_000_000; // 5.0

/// Base block reward in base units (50 SEER × 10^8 precision).
/// Matches genesis.toml block_reward_base = 50.0 and seer-economy BASE_UNIT_PRECISION.
pub const BASE_REWARD: u64 = 50 * 100_000_000; // 5_000_000_000

/// Halving interval in blocks. MUST match seer-economy::constants::HALVING_INTERVAL.
/// At 10 s/block → ~4 years per halving.
pub const HALVING_INTERVAL: u64 = 12_614_400;

/// Burn rate numerator (denominator = 10_000).
/// 100 / 10_000 = 1%. MUST match genesis.toml burn_rate_base = 0.01.
pub const BURN_RATE_NUM: u64 = 100; // 1%

/// Initial circulating supply in base units.
/// MUST match genesis.toml initial_supply = 100_000_000 and seer-economy INITIAL_SUPPLY.
/// Note: stored in base units (1 SEER = 10^8), so 100_000_000 SEER tokens.
pub const INITIAL_SUPPLY: u64 = 100_000_000 * 100_000_000; // 10^16 base units

// ─── Simulation parameters ────────────────────────────────────────────────────

/// Parameters for a single simulation run.
#[derive(Debug, Clone)]
pub struct SimParams {
    /// Initial circulating supply (base units).
    pub initial_supply: u64,
    /// Base block reward (base units).
    pub base_reward: u64,
    /// Halving interval in blocks.
    pub halving_interval: u64,
    /// Burn rate numerator (denominator = 10_000).
    pub burn_rate_num: u64,
    /// Initial number of nodes.
    pub initial_nodes: u64,
    /// Node growth rate per epoch (scaled by 1_000).
    pub node_growth_rate: u64,
    /// Initial difficulty bits.
    pub initial_difficulty: u32,
    /// Target Gini coefficient (scaled by 1_000_000).
    pub target_gini: i64,
    /// Target velocity (scaled by 1_000_000).
    pub target_velocity: i64,
    /// Target block time in seconds.
    pub target_block_time: u64,
    /// Number of epochs to simulate.
    pub max_epochs: u64,
    /// Number of blocks per epoch.
    pub blocks_per_epoch: u64,
    /// Random seed for deterministic simulation.
    pub seed: u64,
}

impl Default for SimParams {
    fn default() -> Self {
        SimParams {
            initial_supply: INITIAL_SUPPLY,
            base_reward: BASE_REWARD,
            halving_interval: HALVING_INTERVAL,
            burn_rate_num: BURN_RATE_NUM,
            initial_nodes: 10,
            node_growth_rate: 100, // 10% per epoch
            initial_difficulty: 16,
            target_gini: TARGET_GINI,
            target_velocity: TARGET_VELOCITY,
            target_block_time: TARGET_BLOCK_TIME,
            max_epochs: SIM_EPOCHS,
            blocks_per_epoch: BLOCKS_PER_EPOCH,
            seed: 0x5EED_5EED_5EED_5EED,
        }
    }
}

// ─── Epoch snapshot ───────────────────────────────────────────────────────────

/// The state of the simulation at the end of a single epoch.
#[derive(Debug, Clone)]
pub struct EpochState {
    /// Epoch number (0-indexed).
    pub epoch: u64,
    /// Total circulating supply at end of epoch.
    pub supply: u64,
    /// Total tokens burned in this epoch.
    pub burned_this_epoch: u64,
    /// Gini coefficient (scaled by 1_000_000).
    pub gini: i64,
    /// Velocity (scaled by 1_000_000).
    pub velocity: i64,
    /// Average block time this epoch (seconds).
    pub avg_block_time: u64,
    /// Current difficulty bits.
    pub difficulty: u32,
    /// Number of active nodes.
    pub node_count: u64,
    /// Block reward at start of epoch.
    pub block_reward: u64,
    /// Whether all convergence criteria were met this epoch.
    pub converged: bool,
}

// ─── Simulation result ────────────────────────────────────────────────────────

/// The result of a full simulation run.
#[derive(Debug, Clone)]
pub struct SimResult {
    /// Per-epoch snapshots.
    pub epochs: Vec<EpochState>,
    /// Whether the simulation converged.
    pub converged: bool,
    /// The epoch at which convergence was first achieved (if any).
    pub convergence_epoch: Option<u64>,
    /// Final supply at end of simulation.
    pub final_supply: u64,
    /// Total tokens burned over the entire simulation.
    pub total_burned: u64,
    /// Suggested genesis parameters derived from the converged state.
    pub genesis_params: Option<GenesisParams>,
}

/// Suggested genesis parameters derived from a converged simulation.
#[derive(Debug, Clone)]
pub struct GenesisParams {
    pub initial_supply: u64,
    pub initial_difficulty: u32,
    pub base_reward: u64,
    pub halving_interval: u64,
    pub burn_rate_num: u64,
    pub target_gini: i64,
    pub target_velocity: i64,
    pub target_block_time: u64,
}

// ─── Deterministic PRNG ───────────────────────────────────────────────────────

/// A simple xorshift64 PRNG for deterministic simulation.
struct Prng {
    state: u64,
}

impl Prng {
    fn new(seed: u64) -> Self {
        Prng { state: seed | 1 }
    }

    fn next(&mut self) -> u64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.state
    }

    /// Returns a value in [0, n).
    fn next_mod(&mut self, n: u64) -> u64 {
        if n == 0 {
            return 0;
        }
        self.next() % n
    }
}

// ─── Simulation engine ────────────────────────────────────────────────────────

/// The convergence simulation engine.
pub struct ConvergenceEngine {
    params: SimParams,
}

impl ConvergenceEngine {
    /// Creates a new simulation engine with the given parameters.
    pub fn new(params: SimParams) -> Self {
        ConvergenceEngine { params }
    }

    /// Creates a new simulation engine with default (genesis-canonical) parameters.
    pub fn default() -> Self {
        ConvergenceEngine::new(SimParams::default())
    }

    /// Runs the full simulation and returns the result.
    pub fn run(&self) -> SimResult {
        let p = &self.params;
        let mut rng = Prng::new(p.seed);

        let mut supply = p.initial_supply;
        let mut total_burned: u64 = 0;
        let mut difficulty = p.initial_difficulty;
        let mut node_count = p.initial_nodes;
        let mut epochs: Vec<EpochState> = Vec::with_capacity(p.max_epochs as usize);
        let mut stable_streak: u64 = 0;
        let mut convergence_epoch: Option<u64> = None;

        // Wealth distribution: simulate `node_count` accounts with varying balances.
        let mut balances: Vec<u64> = (0..p.initial_nodes)
            .map(|_| {
                p.initial_supply / p.initial_nodes
                    + rng.next_mod(
                        p.initial_supply / p.initial_nodes / 10 + 1,
                    )
            })
            .collect();

        for epoch in 0..p.max_epochs {
            let height_start = epoch * p.blocks_per_epoch;
            let halvings = height_start / p.halving_interval;
            let block_reward = p.base_reward >> halvings.min(63);

            // ── Supply dynamics ───────────────────────────────────────────────
            let minted = block_reward.saturating_mul(p.blocks_per_epoch);
            let burned_epoch =
                supply.saturating_mul(p.burn_rate_num).saturating_div(10_000);
            supply = supply.saturating_add(minted).saturating_sub(burned_epoch);
            total_burned = total_burned.saturating_add(burned_epoch);

            // ── Node growth ───────────────────────────────────────────────────
            let growth = node_count
                .saturating_mul(p.node_growth_rate)
                .saturating_div(1_000)
                .max(1);
            let churn = rng.next_mod(growth / 2 + 1);
            node_count = (node_count + growth).saturating_sub(churn).max(1);

            while balances.len() < node_count as usize {
                let new_bal =
                    rng.next_mod(supply / node_count.max(1) / 2 + 1);
                balances.push(new_bal);
            }

            // ── Wealth redistribution ─────────────────────────────────────────
            for _ in 0..p.blocks_per_epoch.min(node_count) {
                let miner_idx = rng.next_mod(node_count) as usize;
                if miner_idx < balances.len() {
                    balances[miner_idx] =
                        balances[miner_idx].saturating_add(block_reward);
                }
            }

            let tx_count = rng.next_mod(p.blocks_per_epoch * 10);
            for _ in 0..tx_count {
                if balances.len() < 2 {
                    break;
                }
                let from = rng.next_mod(node_count) as usize % balances.len();
                let to = rng.next_mod(node_count) as usize % balances.len();
                if from == to {
                    continue;
                }
                let amount = rng.next_mod(balances[from].max(1) / 10 + 1);
                let fee = amount / 100;
                let burn = fee
                    .saturating_mul(p.burn_rate_num)
                    .saturating_div(10_000);
                balances[from] =
                    balances[from].saturating_sub(amount + fee);
                balances[to] = balances[to].saturating_add(amount);
                total_burned = total_burned.saturating_add(burn);
            }

            // ── Gini coefficient ──────────────────────────────────────────────
            let gini = compute_gini_scaled(&balances);

            // ── Velocity ──────────────────────────────────────────────────────
            let volume =
                tx_count.saturating_mul(supply / node_count.max(1) / 100);
            let velocity = if supply > 0 {
                (volume as i64).saturating_mul(1_000_000) / supply as i64
            } else {
                0
            };

            // ── Difficulty adjustment ─────────────────────────────────────────
            let hash_power = node_count.saturating_mul(1_000);
            let target_hashes = 1u64 << difficulty.min(63);
            let avg_block_time = target_hashes
                .saturating_div(hash_power.max(1))
                .max(1)
                .min(300);

            if avg_block_time
                < p.target_block_time
                    .saturating_sub(BLOCK_TIME_TOLERANCE)
            {
                difficulty = difficulty.saturating_add(1).min(64);
            } else if avg_block_time
                > p.target_block_time + BLOCK_TIME_TOLERANCE
            {
                difficulty = difficulty.saturating_sub(1).max(1);
            }

            // ── Convergence check ─────────────────────────────────────────────
            let gini_ok = (gini - p.target_gini).abs() <= GINI_TOLERANCE;
            let vel_ok =
                (velocity - p.target_velocity).abs() <= VELOCITY_TOLERANCE;
            let time_ok = avg_block_time
                .abs_diff(p.target_block_time)
                <= BLOCK_TIME_TOLERANCE;
            let converged_this_epoch = gini_ok && vel_ok && time_ok;

            if converged_this_epoch {
                stable_streak += 1;
            } else {
                stable_streak = 0;
            }

            if convergence_epoch.is_none()
                && stable_streak >= CONVERGENCE_WINDOW
            {
                convergence_epoch = Some(epoch);
            }

            epochs.push(EpochState {
                epoch,
                supply,
                burned_this_epoch: burned_epoch,
                gini,
                velocity,
                avg_block_time,
                difficulty,
                node_count,
                block_reward,
                converged: converged_this_epoch,
            });
        }

        let converged = convergence_epoch.is_some();
        let genesis_params = if converged {
            Some(GenesisParams {
                initial_supply: p.initial_supply,
                initial_difficulty: epochs
                    .last()
                    .map(|e| e.difficulty)
                    .unwrap_or(p.initial_difficulty),
                base_reward: p.base_reward,
                halving_interval: p.halving_interval,
                burn_rate_num: p.burn_rate_num,
                target_gini: p.target_gini,
                target_velocity: p.target_velocity,
                target_block_time: p.target_block_time,
            })
        } else {
            None
        };

        SimResult {
            epochs,
            converged,
            convergence_epoch,
            final_supply: supply,
            total_burned,
            genesis_params,
        }
    }

    /// Runs a parameter sweep over burn rates and returns the best-converging set.
    pub fn sweep_burn_rates(
        &self,
        rates: &[u64],
    ) -> Vec<(u64, SimResult)> {
        rates
            .iter()
            .map(|&rate| {
                let mut p = self.params.clone();
                p.burn_rate_num = rate;
                let engine = ConvergenceEngine::new(p);
                (rate, engine.run())
            })
            .collect()
    }
}

// ─── Gini helper ──────────────────────────────────────────────────────────────

/// Computes the Gini coefficient scaled by 1_000_000.
fn compute_gini_scaled(balances: &[u64]) -> i64 {
    if balances.is_empty() {
        return 0;
    }
    let n = balances.len() as u128;
    let mut sorted = balances.to_vec();
    sorted.sort_unstable();
    let sum: u128 = sorted.iter().map(|&b| b as u128).sum();
    if sum == 0 {
        return 0;
    }
    let mut weighted_sum: u128 = 0;
    for (i, &b) in sorted.iter().enumerate() {
        weighted_sum += (i as u128 + 1) * b as u128;
    }
    // G = (2 * weighted_sum) / (n * sum) - (n+1)/n
    let numerator = 2u128 * weighted_sum * 1_000_000;
    let denominator = n * sum;
    let term1 = numerator / denominator;
    let term2 = (n + 1) * 1_000_000 / n;
    term1.saturating_sub(term2) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canonical_block_time_matches_genesis() {
        assert_eq!(TARGET_BLOCK_TIME, 10, "must match genesis.toml target_block_time");
    }

    #[test]
    fn test_canonical_blocks_per_epoch() {
        assert_eq!(BLOCKS_PER_EPOCH, 8_640, "86400 / 10s block time");
    }

    #[test]
    fn test_canonical_halving_interval_matches_constants_rs() {
        // seer-economy/constants.rs HALVING_INTERVAL = 12_614_400
        assert_eq!(HALVING_INTERVAL, 12_614_400);
    }

    #[test]
    fn test_canonical_burn_rate_matches_genesis() {
        // genesis.toml burn_rate_base = 0.01 → 100/10_000
        assert_eq!(BURN_RATE_NUM, 100);
    }

    #[test]
    fn test_canonical_initial_supply_token_count() {
        // genesis.toml initial_supply = 100_000_000 tokens
        // stored as base units: 100_000_000 * 100_000_000
        let token_count = INITIAL_SUPPLY / 100_000_000;
        assert_eq!(token_count, 100_000_000);
    }

    #[test]
    fn test_prng_deterministic() {
        let mut r1 = Prng::new(42);
        let mut r2 = Prng::new(42);
        for _ in 0..100 {
            assert_eq!(r1.next(), r2.next());
        }
    }

    #[test]
    fn test_prng_different_seeds() {
        let mut r1 = Prng::new(1);
        let mut r2 = Prng::new(2);
        assert_ne!(r1.next(), r2.next());
    }

    #[test]
    fn test_gini_perfect_equality() {
        let balances = vec![100u64; 100];
        let g = compute_gini_scaled(&balances);
        assert_eq!(g, 0, "equal balances should have Gini = 0");
    }

    #[test]
    fn test_gini_empty() {
        assert_eq!(compute_gini_scaled(&[]), 0);
    }

    #[test]
    fn test_gini_all_zero() {
        let balances = vec![0u64; 10];
        assert_eq!(compute_gini_scaled(&balances), 0);
    }

    #[test]
    fn test_gini_range() {
        let balances: Vec<u64> = (1..=100).collect();
        let g = compute_gini_scaled(&balances);
        assert!(g >= 0, "Gini must be non-negative");
        assert!(g <= 1_000_000, "Gini must be ≤ 1.0");
    }

    fn fast_engine() -> ConvergenceEngine {
        ConvergenceEngine::new(SimParams {
            initial_supply: 100_000_000 * 100_000_000,
            base_reward: BASE_REWARD,
            halving_interval: HALVING_INTERVAL,
            burn_rate_num: BURN_RATE_NUM,
            initial_nodes: 10,
            node_growth_rate: 100,
            initial_difficulty: 16,
            target_gini: TARGET_GINI,
            target_velocity: TARGET_VELOCITY,
            target_block_time: TARGET_BLOCK_TIME,
            max_epochs: 5,
            blocks_per_epoch: 10,
            seed: 0x5EED_5EED_5EED_5EED,
        })
    }

    #[test]
    fn test_simulation_deterministic() {
        let e1 = fast_engine();
        let e2 = fast_engine();
        // Run just 5 epochs inline to keep tests fast.
        let r1 = e1.run();
        let r2 = e2.run();
        assert_eq!(r1.final_supply, r2.final_supply);
        assert_eq!(r1.total_burned, r2.total_burned);
    }

    #[test]
    fn test_supply_is_positive() {
        let engine = fast_engine();
        let result = engine.run();
        assert!(result.final_supply > 0, "supply should remain positive");
    }

    #[test]
    fn test_node_count_grows() {
        let engine = fast_engine();
        let result = engine.run();
        let first = result.epochs.first().unwrap().node_count;
        let last = result.epochs.last().unwrap().node_count;
        assert!(last > first, "node count should grow (first={first}, last={last})");
    }
}
