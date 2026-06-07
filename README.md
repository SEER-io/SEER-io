# SEER Network

SEER is a decentralized proof-of-work cryptocurrency network coordinated via Telegram bot nodes. It features an Oracle-enhanced economic model, serverless edge mining on Cloudflare, and automated onboarding.

## Vision
SEER leverages the massive reach of Telegram and the speed of the Cloudflare edge to create a "Network in Your Pocket." It is designed to be the world's most accessible PoW network, where running a node is as simple as talking to a bot.

## Core Pillars
1. **Node-as-a-Bot**: Telegram bots act as the control interface for decentralized nodes.
2. **Edge Mining**: Proof-of-Work computation runs on serverless Cloudflare Workers.
3. **Oracle Tokenomics**: Dynamic burn rates, staking attractors, and market cap proxies derived from real-time network velocity and Gini stability.
4. **Ed25519 Identity**: Every node is a cryptographically secured wallet using industry-standard Ed25519 keypairs.

## Economic Model (The Oracle)
The SEER economy is governed by a Hyper-Dimensional Oracle that monitors:
- **Velocity**: The rate at which SEER moves through the network.
- **Sentiment**: A composite signal of network growth and demand.
- **Gini Index**: Real-time measurement of wealth distribution.
- **Staking Lock**: An organic distribution attractor that offsets volatility.

## Getting Started
To join the network as an operator:
1. **Fork this repository.**
2. **Create a Telegram Bot** via [@BotFather](https://t.me/BotFather).
3. **Run the setup script**:
   ```bash
   ./scripts/setup-ci.sh
   ```
4. **Enable Subdomains** in your Cloudflare dashboard as instructed by the script.

## Recent Achievements (June 2026)
- ✅ **Network Live:** The SEER Coordinator is officially operational on Cloudflare Workers.
- ✅ **Hardened Identity:** All nodes now use real Ed25519 keypairs and ADNL node IDs.
- ✅ **Strict Verification:** The 92-byte binary block protocol is live, ensuring cryptographic honesty.
- ✅ **SEER Earnings:** Nodes now earn 50 SEER per block, tracked via their integrated edge wallets.
- ✅ **Fractal Dashboard:** A sleek Telegram Mini App with real-time fractal mining visuals and USD valuation.
- ✅ **Global Explorer:** [SEER Explorer](https://seer-explorer.toon-satoshi.workers.dev/) live with Oracle metrics.

---\
*SEER: The Network in Your Pocket.*
