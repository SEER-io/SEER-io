# SEER Network Troubleshooting Guide

This guide covers common issues encountered by operators when deploying SEER Network nodes on Cloudflare.

---

## 🛑 Common Questions & Solutions

### Q: Why does the Global Explorer show "Sync Error" or "Scanning..." forever?
**A:** This usually means the Explorer cannot talk to the Coordinator.
1. **Check the Subdomain:** Go to your Cloudflare Dashboard -> `seer-coordinator` -> Settings -> Triggers. Ensure the `workers.dev` subdomain is **ENABLED**.
2. **Internal Blocking:** Ensure the `COORDINATOR` Service Binding is correctly set in your bot worker settings. This allows the bot to talk to the coordinator internally.

---

### Q: My Telegram Bot is unresponsive to `/start`. Why?
**A:** Telegram messages might be blocked or the worker is unreachable.
1. **Check the Subdomain:** Ensure `seer-node-001` has its `workers.dev` subdomain **ENABLED** in the Cloudflare dashboard.
2. **Polling Fallback:** We have implemented a polling fallback. If webhooks are blocked, the bot will still process messages every 60 seconds during its mining cycle. Wait 1 minute and try again.

---

### Q: Why is my SEER balance still 0.00?
**A:** SEER is earned by successfully mining a block.
1. **Check the Log:** Look at the "Last:" line in your Dashboard. If it says "Cycle finished. No block found," it means you haven't won the lottery yet.
2. **Difficulty:** The current difficulty is 16-bit. On average, it takes about 65,000 hashes to find a block. Your node tries 50,000-100,000 per minute. You should earn tokens within a few minutes of active mining.

---

### Q: What is the "Market Cap Proxy" and "Estimated Value"?
**A:** This is a value derived from the **Oracle Tokenomics**.
- It is an algorithmic estimation based on network velocity, scarcity, and distribution stability.
- As the network grows (more nodes, more blocks), the Market Cap Proxy increases, which in turn increases the USD value of your earned SEER.

---

### Q: How do I backup my Wallet?
**A:** Your wallet (Ed25519) is stored in the **`BOT_STATE`** KV namespace under the key `identity`.
- To "backup," you can copy the value of this key from the Cloudflare Dashboard.
- **Warning:** Never share your `privateKeyBase64` with anyone.

### Q: What happened to my old balance? (The June 7 Reset)
**A:** On June 7th, 2026, the network was reset to **Block 0** to finalize the cryptographic protocol.
- All previous balances were cleared.
- This was a one-time event to transition from the "Sketch" phase to the "Honest" phase.
- Moving forward, the chain state is protected by strict SHA-256d verification and Ed25519 signatures.

---

## 🛠️ Expert Tip: The Pattern
If you encounter a new error, analyze it using the **Pattern**:
1. **Research:** Look at the logs (`wrangler tail`).
2. **Strategy:** Formulate a small, specific change.
3. **Execution:** Apply the fix and **Validate** immediately.

*Always validate before assuming a fix worked!*
