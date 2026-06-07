const BOT_TOKEN = "8951904080:AAFbh5R5F1bZz-am9BF0F2drcqxMuTQI6RM";
const COORDINATOR_URL = "https://seer-coordinator.toon-satoshi.workers.dev";
const MASTER_CHANNEL_ID = "-1003997728534"; 

export default {
  async fetch(request, env, ctx) {
    const url = new URL(request.url);

    if (request.method === "POST" && url.pathname === "/telegram-webhook") {
      const update = await request.json();
      return handleTelegramUpdate(update, env);
    }

    if (url.pathname === "/status") {
      try {
        const settings = await env.BOT_STATE.get("settings", { type: "json" }) || { mining_enabled: true, node_name: "SEER Node 001" };
        const lastLog = await env.BOT_STATE.get("last_mining_log") || "No logs yet.";
        const identity = await getOrCreateIdentity(env);
        
        let globalState = { latest_block: 0, velocity: 0.002, staking_ratio: 0.15, total_supply: 100000000, market_cap: 0 };
        let engines = [];
        let total_miner_blocks = 0;

        // BATCH REQUESTS TO REDUCE SUBREQUESTS
        try {
          const res = env.COORDINATOR ? await env.COORDINATOR.fetch(new Request("https://coordinator/network-state")) : await fetch(COORDINATOR_URL + "/network-state");
          globalState = await res.json();
          
          const minerStatsRes = env.COORDINATOR ? await env.COORDINATOR.fetch(new Request("https://coordinator/miner-stats?id=" + identity.adnl_id)) : await fetch(COORDINATOR_URL + "/miner-stats?id=" + identity.adnl_id);
          total_miner_blocks = (await minerStatsRes.json()).blocks || 0;
          
          engines = globalState.cloud_engines > 0 ? ['Cloud'] : [];
          if (globalState.local_engines > 0) engines.push('Local');
        } catch (e) {
            console.error("Global fetch failed", e);
        }

        const mempoolList = await env.BOT_STATE.list({ prefix: "mempool:", limit: 1 });

        return new Response(JSON.stringify({ 
          blocks_mined: total_miner_blocks, 
          earned_seer: total_miner_blocks * 50,
          ...settings, 
          node_id: identity.adnl_id, 
          public_key: identity.publicKeyHex,
          last_log: lastLog, 
          active_engines: engines,
          global_velocity: globalState.velocity,
          global_staking: globalState.staking_ratio,
          global_mcap: globalState.market_cap,
          height: globalState.latest_block,
          mempool_size: mempoolList.keys.length
        }), { headers: { "Content-Type": "application/json", "Access-Control-Allow-Origin": "*" } });
      } catch (e) {
        return new Response(JSON.stringify({ error: e.message }), { status: 500, headers: { "Access-Control-Allow-Origin": "*" } });
      }
    }

    if (request.method === "POST" && url.pathname === "/update-settings") {
      const newSettings = await request.json();
      const currentSettings = await env.BOT_STATE.get("settings", { type: "json" }) || { mining_enabled: true, node_name: "SEER Node 001" };
      await env.BOT_STATE.put("settings", JSON.stringify({ ...currentSettings, ...newSettings }));
      return new Response(JSON.stringify({ success: true }), { headers: { "Content-Type": "application/json", "Access-Control-Allow-Origin": "*" } });
    }

    if (url.pathname === "/" || url.pathname === "/index.html") {
      return new Response(generateDashboardHTML(), { headers: { "Content-Type": "text/html" } });
    }

    if (url.pathname === "/mine-test") {
      const result = await performMining(env);
      return new Response(JSON.stringify({ result }), { headers: { "Content-Type": "application/json" } });
    }

    if (request.method === "POST" && url.pathname === "/redeem") {
      try {
        const { amount, ton_address } = await request.json();
        const state = await env.BOT_STATE.get("node_state", { type: "json" }) || { earned_seer: 0 };
        if ((state.earned_seer || 0) < amount) return new Response(JSON.stringify({ error: "Insufficient balance" }), { status: 400 });
        state.earned_seer -= amount;
        await env.BOT_STATE.put("node_state", JSON.stringify(state));
        const burn_id = Array.from(crypto.getRandomValues(new Uint8Array(16))).map(b => b.toString(16).padStart(2, '0')).join('');
        await env.BOT_STATE.put("redeem:" + burn_id, JSON.stringify({ burn_id, amount, ton_address, timestamp: Date.now(), status: "pending" }));
        return new Response(JSON.stringify({ success: true, burn_id }), { headers: { "Content-Type": "application/json", "Access-Control-Allow-Origin": "*" } });
      } catch (e) { return new Response(JSON.stringify({ error: e.message }), { status: 500 }); }
    }

    return new Response("SEER Bot Node Live");
  },

  async scheduled(event, env, ctx) {
    const settings = await env.BOT_STATE.get("settings", { type: "json" }) || { mining_enabled: true };
    const identity = await getOrCreateIdentity(env);
    const engineType = typeof process !== 'undefined' ? 'Local' : 'Cloud';
    
    if (engineType === 'Local' || Math.random() < 0.2) {
      const hbBody = JSON.stringify({ miner_id: identity.adnl_id, engine_type: engineType });
      if (env.COORDINATOR) ctx.waitUntil(env.COORDINATOR.fetch(new Request("https://coordinator/heartbeat", { method: 'POST', body: hbBody })));
      else ctx.waitUntil(fetch(COORDINATOR_URL + "/heartbeat", { method: 'POST', headers: { "Content-Type": "application/json" }, body: hbBody }));
    }
    
    ctx.waitUntil(pollTelegramUpdates(env));
    if (settings.mining_enabled) ctx.waitUntil(performMining(env));
  }
};

async function signTransaction(env, recipient, amount, fee) {
  const identity = await getOrCreateIdentity(env);
  const state = await env.BOT_STATE.get("node_state", { type: "json" }) || { nonce: 0 };
  const nonce = (state.nonce || 0) + 1;
  const binaryDer = Uint8Array.from(atob(identity.privateKeyBase64), c => c.charCodeAt(0));
  const privateKey = await crypto.subtle.importKey("pkcs8", binaryDer, { name: "Ed25519", namedCurve: "Ed25519" }, true, ["sign"]);
  const txData = identity.publicKeyHex + recipient + amount + fee + nonce;
  const sig = await crypto.subtle.sign({ name: "Ed25519" }, privateKey, new TextEncoder().encode(txData));
  const tx = { sender: identity.publicKeyHex, recipient, amount, fee, nonce, signature: bytesToHex(new Uint8Array(sig)) };
  state.nonce = nonce;
  await env.BOT_STATE.put("node_state", JSON.stringify(state));
  return tx;
}

async function verifyTransaction(tx) {
  try {
    const publicKey = await crypto.subtle.importKey("raw", hexToBytes(tx.sender), { name: "Ed25519", namedCurve: "Ed25519" }, true, ["verify"]);
    const txData = tx.sender + tx.recipient + tx.amount + tx.fee + tx.nonce;
    return await crypto.subtle.verify({ name: "Ed25519" }, publicKey, hexToBytes(tx.signature), new TextEncoder().encode(txData));
  } catch (e) { return false; }
}

async function handleTelegramUpdate(update, env) {
  if (update.channel_post) {
    const text = update.channel_post.text;
    if (text && text.startsWith("SEER_TX:")) {
      try {
        const tx = JSON.parse(text.replace("SEER_TX:", "").trim());
        if (await verifyTransaction(tx)) {
           const txHash = bytesToHex(new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(JSON.stringify(tx)))));
           await env.BOT_STATE.put("mempool:" + txHash, JSON.stringify(tx));
        }
      } catch (e) {}
    }
    return new Response("OK");
  }

  if (!update.message || !update.message.text) return new Response("OK");
  const chatId = update.message.chat.id;
  const text = update.message.text;

  if (text === "/start") {
    await sendTgMessage(chatId, "👁️ SEER Node Bot v1.0.0\n\nWelcome operator!\n\n/send <address> <amount>\n/status - Wallet details\n/mempool - View synced txs");
  } else if (text.startsWith("/send")) {
    const parts = text.split(" ");
    if (parts.length < 3) return sendTgMessage(chatId, "Usage: /send <address> <amount>");
    const tx = await signTransaction(env, parts[1], parseInt(parts[2]), 1);
    await announceToMasterChannel("SEER_TX: " + JSON.stringify(tx));
    await sendTgMessage(chatId, "✅ Transaction signed and broadcast to SEER Mempool.");
  } else if (text === "/mempool") {
    const list = await env.BOT_STATE.list({ prefix: "mempool:" });
    await sendTgMessage(chatId, "📦 LOCAL MEMPOOL\nPending TXs: " + list.keys.length + "\n\nSynced via the Master Channel.");
  } else if (text === "/apply") {
    const identity = await getOrCreateIdentity(env);
    await announceToMasterChannel("🙋 <b>PROMOTION REQUEST</b>\nNode: <code>" + identity.adnl_id + "</code>");
    await sendTgMessage(chatId, "✅ Request sent.");
  } else if (text === "/status") {
    const identity = await getOrCreateIdentity(env);
    await sendTgMessage(chatId, "📊 NODE STATUS\nIdentity: " + identity.adnl_id);
  } else if (text === "/mine") {
    await performMining(env);
    await sendTgMessage(chatId, "⛏️ Manual mine attempt finished.");
  }
  return new Response("OK");
}

async function sendTgMessage(chatId, text) {
  await fetch("https://api.telegram.org/bot" + BOT_TOKEN + "/sendMessage", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ chat_id: chatId, text: text }) });
}

async function announceToMasterChannel(text) {
  try {
    await fetch("https://api.telegram.org/bot" + BOT_TOKEN + "/sendMessage", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ chat_id: MASTER_CHANNEL_ID, text: text, parse_mode: "HTML" }) });
  } catch (e) {}
}

async function getOrCreateIdentity(env) {
  let stored = await env.BOT_STATE.get("identity", { type: "json" });
  if (stored) return stored;
  const keyPair = await crypto.subtle.generateKey({ name: "Ed25519", namedCurve: "Ed25519" }, true, ["sign", "verify"]);
  const publicKey = await crypto.subtle.exportKey("raw", keyPair.publicKey);
  const privateKey = await crypto.subtle.exportKey("pkcs8", keyPair.privateKey);
  const publicKeyHex = bytesToHex(new Uint8Array(publicKey));
  const adnlHash = await crypto.subtle.digest("SHA-256", publicKey);
  const adnl_id = bytesToHex(new Uint8Array(adnlHash)).slice(0, 24);
  const identity = { adnl_id, publicKeyHex, privateKeyBase64: btoa(String.fromCharCode(...new Uint8Array(privateKey))) };
  await env.BOT_STATE.put("identity", JSON.stringify(identity));
  await announceToMasterChannel("🛰 <b>NEW MINER ONLINE</b>\nNode ID: <code>" + adnl_id + "</code>");
  return identity;
}

async function performMining(env) {
  const engineType = typeof process !== 'undefined' ? 'Local' : 'Cloud';
  try {
    const identity = await getOrCreateIdentity(env);
    const res = env.COORDINATOR ? await env.COORDINATOR.fetch(new Request("https://coordinator/network-state")) : await fetch(COORDINATOR_URL + "/network-state");
    const netState = await res.json();
    let currentHeight = 0;
    if (netState.latest_block !== undefined && netState.latest_block !== "Genesis") {
      currentHeight = parseInt(netState.latest_block);
    }
    if (isNaN(currentHeight)) currentHeight = 0;
    const targetHeight = currentHeight + 1;
    const prevHash = hexToBytes(netState.latest_hash || "0000000000000000000000000000000000000000000000000000000000000000");
    
    const mempoolList = await env.BOT_STATE.list({ prefix: "mempool:", limit: 5 });
    const txs = [];
    for (const key of mempoolList.keys) txs.push(await env.BOT_STATE.get(key.name, { type: "json" }));
    const txRoot = bytesToHex(new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(JSON.stringify(txs)))));

    const buffer = new ArrayBuffer(92);
    const view = new DataView(buffer);
    view.setBigUint64(0, BigInt(targetHeight), true);
    new Uint8Array(buffer).set(prevHash, 8);
    new Uint8Array(buffer).set(hexToBytes(txRoot), 40);
    view.setUint32(80, 16, true);

    for (let i = 0; i < 50000; i++) {
      const nonce = BigInt(Math.floor(Math.random() * 2000000000));
      const timestamp = BigInt(Math.floor(Date.now() / 1000));
      view.setBigUint64(72, timestamp, true);
      view.setBigUint64(84, nonce, true);
      const hashArray = new Uint8Array(await crypto.subtle.digest("SHA-256", await crypto.subtle.digest("SHA-256", buffer)));
      
      if (hashArray[0] === 0 && hashArray[1] === 0) {
        const hashHex = bytesToHex(hashArray);
        const submitBody = JSON.stringify({ height: targetHeight, prev_hash: bytesToHex(prevHash), tx_root: txRoot, transactions: txs, timestamp: Number(timestamp), difficulty: 16, nonce: Number(nonce), hash: hashHex, miner: identity.adnl_id });
        let submitRes = env.COORDINATOR ? await env.COORDINATOR.fetch(new Request("https://coordinator/submit-block", { method: "POST", body: submitBody })) : await fetch(COORDINATOR_URL + "/submit-block", { method: "POST", headers: { "Content-Type": "application/json" }, body: submitBody });
        
        if (submitRes.ok) {
          await env.BOT_STATE.put("last_mining_log", "Success! Block mined.");
          await announceToMasterChannel("⛏ <b>BLOCK MINED</b> [" + engineType + "]\nHeight: <b>" + targetHeight + "</b>\nTXs: " + txs.length);
          for (const key of mempoolList.keys) await env.BOT_STATE.delete(key.name);
          return { height: targetHeight, hash: hashHex };
        }
      }
    }
    if (Math.random() < 0.1 || engineType === 'Local') {
      await env.BOT_STATE.put("last_mining_log", "Cycle finished [" + engineType + "]. No block.");
    }
  } catch (e) { await env.BOT_STATE.put("last_mining_log", "Error: " + e.message); }
  return null;
}

function hexToBytes(hex) {
  const bytes = new Uint8Array(hex.length / 2);
  for (let i = 0; i < hex.length; i += 2) bytes[i / 2] = parseInt(hex.substr(i, 2), 16);
  return bytes;
}

function bytesToHex(bytes) {
  return Array.from(bytes).map(b => b.toString(16).padStart(2, '0')).join('');
}

async function pollTelegramUpdates(env) {
  try {
    const offset = await env.BOT_STATE.get("tg_offset") || 0;
    const res = await fetch("https://api.telegram.org/bot" + BOT_TOKEN + "/getUpdates?offset=" + offset + "&timeout=30");
    const data = await res.json();
    if (data.ok && data.result.length > 0) {
      let maxId = offset;
      for (const update of data.result) {
        await handleTelegramUpdate(update, env);
        maxId = Math.max(maxId, update.update_id + 1);
      }
      await env.BOT_STATE.put("tg_offset", maxId.toString());
    }
  } catch (e) {}
}

function generateDashboardHTML() {
  return `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>SEER Node - Dashboard</title>
    <style>
        :root { --neon-blue: #00f2ff; --neon-purple: #bc13fe; --dark-bg: #050505; --panel-bg: #111; }
        body { background: var(--dark-bg); color: #fff; font-family: 'Segoe UI', sans-serif; margin: 0; padding: 15px; display: flex; flex-direction: column; align-items: center; }
        .card { background: var(--panel-bg); border: 1px solid #222; padding: 20px; border-radius: 16px; width: 100%; max-width: 400px; box-shadow: 0 10px 30px rgba(0,0,0,0.5); }
        .header { display: flex; justify-content: space-between; align-items: center; border-bottom: 1px solid #222; padding-bottom: 15px; margin-bottom: 20px; }
        .wallet-pill { background: #1a1a1a; padding: 5px 12px; border-radius: 20px; font-size: 0.7rem; color: var(--neon-blue); border: 1px solid #333; }
        .balance-box { text-align: center; margin-bottom: 25px; }
        .balance-amount { font-size: 2.5rem; font-weight: 800; color: var(--neon-blue); }
        .balance-label { font-size: 0.8rem; opacity: 0.5; text-transform: uppercase; letter-spacing: 1px; }
        .stats-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 15px; margin-bottom: 20px; }
        .stat-item { background: #0a0a0a; padding: 12px; border-radius: 10px; border: 1px solid #222; }
        .stat-l { font-size: 0.65rem; opacity: 0.5; text-transform: uppercase; }
        .stat-v { font-size: 1rem; font-weight: bold; color: var(--neon-blue); }
        .engine-tag { font-size: 0.6rem; padding: 2px 6px; border-radius: 4px; background: #222; margin-right: 5px; color: #fff; }
        .engine-active { background: var(--neon-purple); }
        .console { background: #000; border-radius: 10px; padding: 12px; font-family: 'Courier New', monospace; height: 100px; overflow: hidden; font-size: 0.7rem; color: #00ff00; border: 1px solid #222; word-break: break-all; }
        .btn { background: var(--neon-blue); color: #000; border: none; padding: 12px; border-radius: 8px; font-weight: bold; cursor: pointer; width: 100%; margin-top: 15px; }
        .progress-bar { width: 100%; background: #222; height: 3px; border-radius: 10px; margin-top: 8px; overflow: hidden; }
        .progress-fill { height: 100%; background: var(--neon-blue); width: 0%; transition: width 0.2s; }
        .bridge-pill { background: #330000; color: #ff0000; font-size: 0.6rem; padding: 2px 6px; border-radius: 4px; border: 1px solid #ff0000; margin-left: 5px; }
    </style>
</head>
<body>
    <div class="card">
        <div class="header">
            <span style="font-weight:bold; letter-spacing: 1px;">👁️ SEER NODE</span>
            <div class="wallet-pill" id="wallet-short">0x...</div>
        </div>

        <div class="balance-box">
            <div class="balance-label">Unified Earnings</div>
            <div class="balance-amount" id="earned-seer">0</div>
            <div style="font-size: 0.8rem; opacity: 0.8;">SEER</div>
            <div id="engine-fleet" style="margin-top:10px;"></div>
        </div>

        <div class="stats-grid">
            <div class="stat-item"><div class="stat-l">Block Height</div><div class="stat-v" id="height">0</div></div>
            <div class="stat-item"><div class="stat-l">Mempool</div><div class="stat-v" id="mempool">0</div></div>
        </div>

        <div style="font-size: 0.7rem; opacity: 0.6; margin-bottom: 8px;">ORACLE MINING FEED</div>
        <div class="console" id="mining-console"></div>
        <div class="progress-bar"><div class="progress-fill" id="p-fill"></div></div>
        
        <div id="last-log" style="font-size: 0.6rem; opacity: 0.4; margin-top: 8px;">Waiting for cycle...</div>

        <div style="margin-top: 25px; border-top: 1px solid #222; padding-top: 15px;">
            <div style="display: flex; justify-content: space-between; align-items: center; font-size: 0.8rem;">
                <span style="opacity:0.6">Mining Power (All)</span>
                <input type="checkbox" id="mining-toggle" onchange="saveSettings()">
            </div>
            <button class="btn" onclick="saveSettings()">SAVE SETTINGS</button>
        </div>

        <div style="margin-top: 25px; border-top: 1px solid #222; padding-top: 15px;">
            <div style="font-weight:bold; font-size:0.8rem; margin-bottom:10px; color: var(--neon-purple);">🌉 TON TESTNET BRIDGE</div>
            <input type="text" id="ton-address-input" placeholder="TON Testnet Wallet Address" style="width:100%; background:#000; border:1px solid #222; color:#fff; padding:8px; border-radius:5px; font-size:0.7rem;">
            <button class="btn" style="background: var(--neon-purple); margin-top:10px;" onclick="redeemTokens()">REDEEM SEER JETTONS</button>
        </div>
    </div>

    <script>
        const SYMBOLS = ["▲", "■", "◆", "◉", "·"];
        async function fetchStats() {
            try {
                const res = await fetch('/status');
                const data = await res.json();
                if (data.error) {
                    document.getElementById('last-log').textContent = "Server Error: " + data.error;
                    return;
                }
                document.getElementById('earned-seer').textContent = data.earned_seer || 0;
                document.getElementById('height').textContent = data.height;
                document.getElementById('mempool').textContent = data.mempool_size;
                document.getElementById('wallet-short').textContent = data.public_key.slice(0, 10) + '...';
                document.getElementById('mining-toggle').checked = data.mining_enabled;
                document.getElementById('last-log').textContent = data.last_log;
                
                const fleet = document.getElementById('engine-fleet');
                fleet.innerHTML = '';
                ['Cloud', 'Local'].forEach(type => {
                    const span = document.createElement('span');
                    span.className = 'engine-tag' + (data.active_engines.includes(type) ? ' engine-active' : '');
                    span.textContent = type + ' ENGINE';
                    fleet.appendChild(span);
                });
            } catch(e) {
                document.getElementById('last-log').textContent = "Sync Error: Check Connection";
            }
        }
        async function saveSettings() {
            const enabled = document.getElementById('mining-toggle').checked;
            await fetch('/update-settings', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ mining_enabled: enabled }) });
            fetchStats();
        }

        async function redeemTokens() {
            const tonAddress = document.getElementById('ton-address-input').value;
            const amount = prompt("How many SEER tokens would you like to redeem?");
            if (!amount || isNaN(amount)) return;
            const res = await fetch('/redeem', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ amount: parseInt(amount), ton_address: tonAddress }) });
            const result = await res.json();
            if (result.success) { alert('SUCCESS! Burn ID: ' + result.burn_id); fetchStats(); }
            else { alert("ERROR: " + result.error); }
        }

        function updateConsole() {
            if (!document.getElementById('mining-toggle').checked) return;
            const consoleBox = document.getElementById('mining-console');
            const fill = document.getElementById('p-fill');
            const char = SYMBOLS[Math.floor(Math.random() * SYMBOLS.length)];
            const entry = document.createElement('span');
            entry.style.color = '#' + Math.floor(Math.random()*16777215).toString(16);
            entry.textContent = char;
            consoleBox.appendChild(entry);
            if (consoleBox.childNodes.length > 250) consoleBox.removeChild(consoleBox.firstChild);
            let p = parseFloat(fill.style.width || "0");
            fill.style.width = ((p + 2) % 101) + "%";
        }
        fetchStats();
        setInterval(fetchStats, 5000);
        setInterval(updateConsole, 50);
    </script>
</body>
</html>`;
}
