# SEER Network

SEER is a decentralized proof-of-work cryptocurrency network coordinated via Telegram bot nodes. It features an Oracle-enhanced economic model, serverless edge mining on Cloudflare, and automated onboarding.

## Vision: The Inclusive Network
SEER leverages the massive reach of Telegram and the speed of the Cloudflare edge to create a **"Network in Your Pocket."** It is designed to be the world's most accessible PoW network, breaking the hardware barrier that keeps most people out of the crypto-economy.

### Hybrid Deployment: Cloud or Bare Metal
SEER is unique because it allows for two distinct ways to participate:
1. **Cloud-Native Mining (Zero Hardware):** 
   - You don't need a computer to mine SEER. 
   - With just a **GitHub** and **Cloudflare** account (both free), you can deploy an automated node that mines 24/7 on the internet's edge.
   - If you have a phone and a browser, you can be a miner.
2. **Bare Metal Performance:** 
   - For operators with dedicated hardware or a high-performance **Termux** environment on Android, the local runner provides low-latency execution and direct control.

## Core Pillars
1. **Node-as-a-Bot**: Telegram bots act as the control interface for decentralized nodes.
2. **Edge Mining**: Proof-of-Work computation runs on serverless Cloudflare Workers.
3. **Oracle Tokenomics**: Dynamic metrics derived from real-time network velocity and Gini stability.
4. **Ed25519 Identity**: Every node is a cryptographically secured wallet using Ed25519 keypairs.

## Getting Started
To join the network as an operator:
1. **Fork this repository.**
2. **Create a Telegram Bot** via [@BotFather](https://t.me/BotFather).
3. **Run the setup script**:
   ```bash
   ./scripts/setup.sh
   ```
4. **Enable Subdomains** in your Cloudflare dashboard as instructed by the script.

## Network History: The Genesis Reset (June 7, 2026)
On June 7th, 2026, the network underwent a **"Scorched Earth" Reset**. 
*   **Reason:** After successfully testing the prototype stack to Block 57, the architecture was finalized (Strict 92-byte protocol, Ed25519 identities, and Service Bindings). 
*   **Result:** All previous test blocks were cleared to ensure a clean, honest, and documented launch from **Block 0**.

## Recent Achievements
- ✅ **Genesis Launch:** Network reset and restarted at Block 0 with strict cryptographic verification.
- ✅ **Hybrid Engines:** Support for both Cloudflare Edge and Local Bare Metal mining.
- ✅ **Interactive Explorer:** Live feed showing engine types and block history.
- ✅ **Master Channel:** Global coordination via the SEER Miner Channel.

---
*SEER: The Network in Your Pocket.*
