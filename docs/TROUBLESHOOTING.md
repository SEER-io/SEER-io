# SEER Network Troubleshooting Guide

This guide covers common issues encountered by operators when deploying SEER Network nodes on Cloudflare.

---

## 🛑 Common Questions & Solutions

### Q: Why does the Global Explorer show "Sync Error" or "Scanning..." forever?
**A:** This usually means the Explorer cannot talk to the Coordinator.
1. **Check the Subdomain:** Go to your Cloudflare Dashboard -> `seer-coordinator` -> Settings -> Triggers. Ensure the `workers.dev` subdomain is **ENABLED**.
2. **Missing KV Storage:** Ensure you have a KV namespace named `NETWORK_STATE` created and bound to the `seer-coordinator` worker.
3. **CORS Issues:** If you are testing locally, ensure the coordinator's URL in `index.html` matches your actual worker URL.

---

### Q: My Telegram Bot is unresponsive to `/start`. Why?
**A:** Telegram is trying to send messages to your worker, but the worker is either not there or returning an error.
1. **Check the Subdomain:** Just like the coordinator, ensure `seer-node-001` has its `workers.dev` subdomain **ENABLED** in the Cloudflare dashboard.
2. **Webhook Error:** If it's enabled but still dead, run this command to see the error:
   ```bash
   curl -s "https://api.telegram.org/bot<YOUR_TOKEN>/getWebhookInfo"
   ```
   If it says `404 Not Found`, the worker URL is wrong. If it says `401 Unauthorized`, your `BOT_TOKEN` is wrong.

---

### Q: GitHub Actions are failing with "Uncaught SyntaxError" or "TypeError".
**A:** This is usually due to the worker module format.
1. **ES Module vs Service Worker:** Cloudflare Workers use two different formats. If you are using `export default { fetch... }`, you must use the `--compatibility-flags` or specify the module type in `wrangler.toml`.
2. **Missing Secrets:** Check your GitHub Repository -> Settings -> Secrets -> Actions. You MUST have:
   - `CLOUDFLARE_API_TOKEN`
   - `CLOUDFLARE_ACCOUNT_ID`
   - `BOT_TOKEN`

---

### Q: The Pages deployment fails with "Missing project name" or "Binding errors".
**A:** `wrangler.toml` is very strict.
1. **Name Field:** Ensure `name = "seer-network"` is at the very top of your `wrangler.toml`.
2. **IDs:** Bindings like `kv_namespaces` and `d1_databases` require an `id` (or `database_id`) string. You can find these IDs in the Cloudflare Dashboard under each resource.

---

### Q: How do I manually "Force" a mining cycle?
**A:** Open your bot in Telegram and type `/mine`. The bot will attempt to find a block immediately and send it to the coordinator. If it succeeds, the Explorer will update automatically.

---

## 🛠️ Expert Tip: The Pattern
If you encounter a new error, analyze it using the **Pattern**:
1. **Research:** Look at the logs (`wrangler tail`).
2. **Strategy:** Formulate a small, specific change.
3. **Execution:** Apply the fix and **Validate** immediately.

*Always validate before assuming a fix worked!*
