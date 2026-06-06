//! # Seer Economic Metrics & Sentiment Analysis
//!
//! Tracks real-time network metrics to inform economic adjustments. By monitoring
//! velocity and sentiment-related scalars, the network can proactively adjust
//! difficulty targets and asset burn rates to maintain stability.
//!
//! ## Metrics tracked
//! - **Token Velocity**: rate of asset transfers (transactions per epoch / circulating supply)
//! - **Sentiment Scalar**: composite signal derived from on-chain activity ratios
//! - **Burn Rate**: dynamic burn rate adjusted by velocity and sentiment
//! - **Epoch Statistics**: per-epoch transaction count, volume, and fee totals

use crate::constants::{
    BURN_RATE_BASE_NUM, BURN_RATE_MAX_NUM, RATE_DENOM,
    SENTIMENT_BUFFER_SCALED, SENTIMENT_SCALE,
    VELOCITY_TARGET_SCALED, VELOCITY_SCALE,
};

// ─── Epoch snapshot ───────────────────────────────────────────────────────────

/// A snapshot of on-chain activity for a single epoch (a fixed number of blocks).
#[derive(Debug, Clone, Default)]
pub struct EpochSnapshot {
    /// Block height at the start of this epoch.
    pub start_height: u64,
    /// Block height at the end of this epoch.
    pub end_height: u64,
    /// Total number of transactions processed in this epoch.
    pub tx_count: u64,
    /// Total token volume transferred (in base units).
    pub volume: u64,
    /// Total fees collected (in base units).
    pub fees_collected: u64,
    /// Total tokens burned (in base units).
    pub tokens_burned: u64,
    /// Circulating supply at the end of this epoch (in base units).
    pub circulating_supply: u64,
    /// Number of unique active addresses observed.
    pub active_addresses: u64,
}

impl EpochSnapshot {
    /// Creates a new epoch snapshot.
    pub fn new(
        start_height: u64,
        end_height: u64,
        tx_count: u64,
        volume: u64,
        fees_collected: u64,
        tokens_burned: u64,
        circulating_supply: u64,
        active_addresses: u64,
    ) -> Self {
        Self {
            start_height,
            end_height,
            tx_count,
            volume,
            fees_collected,
            tokens_burned,
            circulating_supply,
            active_addresses,
        }
    }

    /// Returns the number of blocks in this epoch.
    pub fn epoch_length(&self) -> u64 {
        self.end_height.saturating_sub(self.start_height)
    }
}

// ─── Velocity ─────────────────────────────────────────────────────────────────

/// Computes the token velocity for an epoch.
///
/// Velocity = total volume transferred / circulating supply.
/// Scaled by `VELOCITY_SCALE` (e.g., 500 = 5.0).
///
/// Returns 0 if circulating supply is zero.
pub fn compute_velocity(snapshot: &EpochSnapshot) -> u64 {
    if snapshot.circulating_supply == 0 {
        return 0;
    }
    snapshot
        .volume
        .saturating_mul(VELOCITY_SCALE)
        / snapshot.circulating_supply
}

/// Returns `true` if the computed velocity is within the target range
/// (target ± sentiment buffer).
pub fn velocity_in_target(velocity: u64) -> bool {
    let buffer = VELOCITY_TARGET_SCALED * SENTIMENT_BUFFER_SCALED / SENTIMENT_SCALE;
    let low = VELOCITY_TARGET_SCALED.saturating_sub(buffer);
    let high = VELOCITY_TARGET_SCALED + buffer;
    velocity >= low && velocity <= high
}

// ─── Sentiment scalar ─────────────────────────────────────────────────────────

/// A composite sentiment scalar derived from on-chain activity.
///
/// Sentiment ∈ [0, SENTIMENT_SCALE]:
/// - Values above `SENTIMENT_SCALE / 2` indicate positive/bullish network activity.
/// - Values below indicate negative/bearish activity.
///
/// Computed as a weighted combination of:
/// - Active address growth ratio
/// - Fee-to-volume ratio (proxy for demand)
/// - Velocity deviation from target
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SentimentScalar(pub u64);

impl SentimentScalar {
    /// Computes the sentiment scalar from two consecutive epoch snapshots.
    ///
    /// `prev` is the previous epoch; `curr` is the current epoch.
    pub fn compute(prev: &EpochSnapshot, curr: &EpochSnapshot) -> Self {
        let scale = SENTIMENT_SCALE;

        // 1. Active address growth ratio (capped at 2×).
        let addr_ratio = if prev.active_addresses == 0 {
            scale // neutral if no previous data
        } else {
            (curr.active_addresses * scale / prev.active_addresses).min(2 * scale)
        };

        // 2. Fee-to-volume ratio (higher = more demand, capped at scale).
        let fee_ratio = if curr.volume == 0 {
            0
        } else {
            (curr.fees_collected * scale / curr.volume).min(scale)
        };

        // 3. Velocity deviation: how close is velocity to target?
        let velocity = compute_velocity(curr);
        let vel_deviation = if velocity >= VELOCITY_TARGET_SCALED {
            velocity - VELOCITY_TARGET_SCALED
        } else {
            VELOCITY_TARGET_SCALED - velocity
        };
        // Normalise deviation to [0, scale]; small deviation → high score.
        let vel_score = scale.saturating_sub(
            vel_deviation * scale / VELOCITY_TARGET_SCALED.max(1)
        );

        // Weighted average: addr_ratio 40%, fee_ratio 30%, vel_score 30%.
        let raw = (addr_ratio * 40 + fee_ratio * 30 + vel_score * 30) / 100;
        // Normalise to [0, scale].
        SentimentScalar(raw.min(scale))
    }

    /// Returns `true` if sentiment is positive (above neutral midpoint).
    pub fn is_positive(&self) -> bool {
        self.0 > SENTIMENT_SCALE / 2
    }

    /// Returns the raw scalar value.
    pub fn value(&self) -> u64 {
        self.0
    }
}

// ─── Dynamic burn rate ────────────────────────────────────────────────────────

/// Computes the dynamic burn rate numerator (over `RATE_DENOM`) for the next epoch.
///
/// The burn rate is adjusted based on velocity and sentiment:
/// - High velocity + positive sentiment → lower burn (tokens circulating well)
/// - Low velocity + negative sentiment → higher burn (reduce supply to stimulate)
///
/// The result is clamped to `[BURN_RATE_BASE_NUM, BURN_RATE_MAX_NUM]`.
pub fn dynamic_burn_rate(velocity: u64, sentiment: &SentimentScalar) -> u64 {
    let base = BURN_RATE_BASE_NUM;

    // Velocity factor: how far below target is velocity?
    let vel_deficit = if velocity >= VELOCITY_TARGET_SCALED {
        0u64
    } else {
        VELOCITY_TARGET_SCALED - velocity
    };
    // Each unit of deficit adds a fraction of a burn rate point.
    let vel_adjustment = vel_deficit * (BURN_RATE_MAX_NUM - base)
        / VELOCITY_TARGET_SCALED.max(1);

    // Sentiment factor: negative sentiment increases burn.
    let sentiment_adjustment = if sentiment.is_positive() {
        0u64
    } else {
        let negativity = SENTIMENT_SCALE / 2 - sentiment.value().min(SENTIMENT_SCALE / 2);
        negativity * (BURN_RATE_MAX_NUM - base) / (SENTIMENT_SCALE / 2).max(1)
    };

    (base + vel_adjustment + sentiment_adjustment).min(BURN_RATE_MAX_NUM)
}

// ─── Metrics accumulator ──────────────────────────────────────────────────────

/// Accumulates per-block statistics and produces epoch snapshots.
#[derive(Debug, Default)]
pub struct MetricsAccumulator {
    pub start_height: u64,
    pub tx_count: u64,
    pub volume: u64,
    pub fees_collected: u64,
    pub tokens_burned: u64,
    pub active_addresses: std::collections::HashSet<[u8; 32]>,
}

impl MetricsAccumulator {
    /// Creates a new accumulator starting at the given block height.
    pub fn new(start_height: u64) -> Self {
        Self {
            start_height,
            ..Default::default()
        }
    }

    /// Records a single transaction.
    pub fn record_tx(
        &mut self,
        sender: [u8; 32],
        receiver: [u8; 32],
        amount: u64,
        fee: u64,
        burned: u64,
    ) {
        self.tx_count += 1;
        self.volume = self.volume.saturating_add(amount);
        self.fees_collected = self.fees_collected.saturating_add(fee);
        self.tokens_burned = self.tokens_burned.saturating_add(burned);
        self.active_addresses.insert(sender);
        self.active_addresses.insert(receiver);
    }

    /// Finalises the epoch and returns a snapshot.
    pub fn finalise(&self, end_height: u64, circulating_supply: u64) -> EpochSnapshot {
        EpochSnapshot::new(
            self.start_height,
            end_height,
            self.tx_count,
            self.volume,
            self.fees_collected,
            self.tokens_burned,
            circulating_supply,
            self.active_addresses.len() as u64,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_snapshot(volume: u64, supply: u64, fees: u64, addresses: u64) -> EpochSnapshot {
        EpochSnapshot::new(0, 100, 50, volume, fees, 0, supply, addresses)
    }

    #[test]
    fn test_velocity_zero_supply() {
        let snap = make_snapshot(1_000, 0, 10, 5);
        assert_eq!(compute_velocity(&snap), 0);
    }

    #[test]
    fn test_velocity_at_target() {
        // velocity = volume * VELOCITY_SCALE / supply = 5 * 100 / 100 = 5.0 → 500 scaled
        let snap = make_snapshot(5_000, 1_000, 50, 10);
        let v = compute_velocity(&snap);
        assert_eq!(v, VELOCITY_TARGET_SCALED);
        assert!(velocity_in_target(v));
    }

    #[test]
    fn test_velocity_out_of_target() {
        let snap = make_snapshot(100, 1_000_000, 1, 2);
        let v = compute_velocity(&snap);
        assert!(!velocity_in_target(v));
    }

    #[test]
    fn test_sentiment_positive() {
        let prev = make_snapshot(1_000, 10_000, 100, 50);
        let curr = make_snapshot(5_000, 10_000, 500, 100); // growing addresses, high fees
        let s = SentimentScalar::compute(&prev, &curr);
        assert!(s.is_positive(), "sentiment should be positive, got {}", s.value());
    }

    #[test]
    fn test_dynamic_burn_rate_high_velocity_positive_sentiment() {
        let high_velocity = VELOCITY_TARGET_SCALED * 2;
        let positive = SentimentScalar(SENTIMENT_SCALE);
        let rate = dynamic_burn_rate(high_velocity, &positive);
        assert_eq!(rate, BURN_RATE_BASE_NUM, "high velocity + positive sentiment → base burn rate");
    }

    #[test]
    fn test_dynamic_burn_rate_low_velocity_negative_sentiment() {
        let low_velocity = 0u64;
        let negative = SentimentScalar(0);
        let rate = dynamic_burn_rate(low_velocity, &negative);
        assert_eq!(rate, BURN_RATE_MAX_NUM, "low velocity + negative sentiment → max burn rate");
    }

    #[test]
    fn test_accumulator_records_txs() {
        let mut acc = MetricsAccumulator::new(0);
        let alice = [1u8; 32];
        let bob = [2u8; 32];
        acc.record_tx(alice, bob, 1_000, 10, 1);
        acc.record_tx(alice, bob, 2_000, 20, 2);
        let snap = acc.finalise(100, 1_000_000);
        assert_eq!(snap.tx_count, 2);
        assert_eq!(snap.volume, 3_000);
        assert_eq!(snap.fees_collected, 30);
        assert_eq!(snap.tokens_burned, 3);
        assert_eq!(snap.active_addresses, 2); // alice and bob
    }

    #[test]
    fn test_epoch_length() {
        let snap = EpochSnapshot::new(100, 200, 0, 0, 0, 0, 0, 0);
        assert_eq!(snap.epoch_length(), 100);
    }
}
