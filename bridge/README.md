# SEER TON Bridge

The 1:1 Redemption Bridge connects the SEER internal chain to the TON (The Open Network) blockchain.

## Mechanism: Burn-and-Mint
1. **Burn:** A user destroys their earned SEER on the internal ledger using the `/redeem` command on the Node Mini App.
2. **Verify:** A unique Burn ID is generated and recorded.
3. **Mint:** An off-chain relayer detects the burn and triggers a `mint` on the **SeerJettonMaster** contract on TON Testnet.

## Contracts
- **`seer_jetton.tact`**: TEP-74 compliant Jetton implementation written in Tact.
- **Master**: Manages global supply and authorized minting.
- **Wallet**: Sharded individual user balances on the TON network.

---
*Bridging Private Mining to Public Liquidity.*
