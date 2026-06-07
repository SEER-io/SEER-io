# SEER Economy Crate

The "Oracle" of the network. This crate implements the algorithmic tokenomics and metrics tracking that drive the SEER ecosystem.

## Components

### 1. `metrics.rs`
The Hyper-Dimensional Oracle logic.
- **Velocity:** Tracks the circulation speed of SEER tokens.
- **Sentiment:** A composite score derived from network growth and active address expansion.

### 2. `gini.rs`
Calculates the Gini Coefficient of the network in real-time.
- **Purpose:** Measures wealth distribution to prevent extreme centralization.
- **Feedback:** Influences block rewards and burn rates to encourage organic distribution.

### 3. `constants.rs`
Defines the economic limits: max supply, halving intervals, and baseline burn rates.

---
*SEER: An Economy Governed by Mathematics.*
