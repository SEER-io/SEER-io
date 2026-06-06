# SEER Network

SEER is a decentralized proof-of-work cryptocurrency network coordinated via Telegram bot nodes. It features a Cloudflare-hosted global explorer and automated onboarding.

## Vision
SEER leverages the massive reach of Telegram to create a highly accessible, truly decentralized network where every Telegram account can act as a coordinator or a node.

## How to Join the Network

### Prerequisites
- A Linux environment or Termux (for Android).
- A Telegram account.
- `curl`, `unzip`, and `cargo` installed.

### Onboarding Steps
1. **Fork this repository** to your GitHub account.
2. **Create a Telegram Bot**: Message [@BotFather](https://t.me/botfather) to create a new bot and get your `BOT_TOKEN`.
3. **Add Secrets**: In your forked repository, go to `Settings > Secrets and variables > Actions` and add a repository secret named `BOT_TOKEN` with your bot's token.
4. **Register Node**: Go to the `Actions` tab and run the `register-node` workflow manually. This will announce your node to the network.
5. **Run Locally**:
   ```bash
   curl -sSL https://raw.githubusercontent.com/your-username/Seer/main/setup.sh | bash
   ```

## Network Parameters
- **Genesis Supply**: 100,000,000 SEER
- **Block Time**: 10 seconds
- **Block Reward**: 50 SEER
- **Halving Interval**: Every 12,614,400 blocks
- **Burn Rate**: 1% per transaction
- **Identity**: ADNL-style 256-bit (SHA-256 of pubkey)
- **Node Bots**: `@seer_<first6bytes>_bot`

## Explorer
Check the current network state at our [Global Explorer](https://seer-explorer.pages.dev) (Hosted on Cloudflare Pages).

## Architecture
- **seer-core**: Core blockchain logic, blocks, and ledger.
- **seer-crypto**: ADNL identities and zero-channel encryption.
- **seer-consensus**: PoW and vertical patching.
- **seer-telegram**: Telegram bot transport layer.
- **coordinator**: Cloudflare Worker managing node registration and network state.

---
*SEER: The Network in Your Pocket.*
