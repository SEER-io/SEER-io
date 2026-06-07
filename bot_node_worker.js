const BOT_TOKEN = "8951904080:AAFbh5R5F1bZz-am9BF0F2drcqxMuTQI6RM";
const COORDINATOR_URL = "https://seer-coordinator.toon-satoshi.workers.dev";
const MASTER_CHANNEL_ID = "-1003997728534"; 

export default {
  async fetch(request, env, ctx) {
    const url = new URL(request.url);
    await env.BOT_STATE.put("last_request", `${request.method} ${url.pathname} at ${new Date().toISOString()}`);

    if (request.method === "POST" && url.pathname === "/telegram-webhook") {
      const update = await request.json();
      return handleTelegramUpdate(update, env);
    }

    if (url.pathname === "/status") {
      const settings = await env.BOT_STATE.get("settings", { type: "json" }) || { mining_enabled: true, node_name: "SEER Node 001" };
      const lastLog = await env.BOT_STATE.get("last_mining_log") || "No logs yet.";
      const identity = await getOrCreateIdentity(env);
      
      let globalState = { velocity: 0.002, staking_ratio: 0.15, total_supply: 100000000, market_cap: 0 };
      let engines = [];
      let total_miner_blocks = 0;

      try {
        const res = env.COORDINATOR ? await env.COORDINATOR.fetch(new Request("https://coordinator/network-state")) : await fetch(`${COORDINATOR_URL}/network-state`);
        globalState = await res.json();
        
        const engineRes = env.COORDINATOR ? await env.COORDINATOR.fetch(new Request(`https://coordinator/engines?miner_id=${identity.adnl_id}`)) : await fetch(`${COORDINATOR_URL}/engines?miner_id=${identity.adnl_id}`);
        const engineData = await engineRes.json();
        engines = engineData.engines;

        const minerStatsRes = env.COORDINATOR ? await env.COORDINATOR.fetch(new Request(`https://coordinator/miner-stats?id=${identity.adnl_id}`)) : await fetch(`${COORDINATOR_URL}/miner-stats?id=${identity.adnl_id}`);
        const minerStats = await minerStatsRes.json();
        total_miner_blocks = minerStats.blocks;
      } catch (e) {}

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
        height: globalState.latest_block
      }), { 
        headers: { "Content-Type": "application/json", "Access-Control-Allow-Origin": "*" } 
      });
    }

    if (request.method === "POST" && url.pathname === "/update-settings") {
      const newSettings = await request.json();
      const currentSettings = await env.BOT_STATE.get("settings", { type: "json" }) || { mining_enabled: true, node_name: "SEER Node 001" };
      const updated = { ...currentSettings, ...newSettings };
      await env.BOT_STATE.put("settings", JSON.stringify(updated));
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
        if (!state.earned_seer || state.earned_seer < amount) return new Response(JSON.stringify({ error: "Insufficient balance" }), { status: 400 });
        state.earned_seer -= amount;
        await env.BOT_STATE.put("node_state", JSON.stringify(state));
        const burn_id = Array.from(crypto.getRandomValues(new Uint8Array(16))).map(b => b.toString(16).padStart(2, '0')).join('');
        const request_data = { burn_id, amount, ton_address, node_id: (await getOrCreateIdentity(env)).adnl_id, timestamp: Date.now(), status: "pending" };
        await env.BOT_STATE.put(`redeem:${burn_id}`, JSON.stringify(request_data));
        return new Response(JSON.stringify({ success: true, burn_id, message: "Tokens burned. Relayer notified." }), { headers: { "Content-Type": "application/json", "Access-Control-Allow-Origin": "*" } });
      } catch (e) {
        return new Response(JSON.stringify({ error: e.message }), { status: 500 });
      }
    }

    return new Response("SEER Bot Node Live");
  },

  async scheduled(event, env, ctx) {
    const settings = await env.BOT_STATE.get("settings", { type: "json" }) || { mining_enabled: true };
    const identity = await getOrCreateIdentity(env);
    const engineType = typeof process !== 'undefined' ? 'Local' : 'Cloud';

    // 1. Heartbeat (Track engine presence)
    const hbBody = JSON.stringify({ miner_id: identity.adnl_id, engine_type: engineType });
    if (env.COORDINATOR) {
        ctx.waitUntil(env.COORDINATOR.fetch(new Request("https://coordinator/heartbeat", { method: 'POST', body: hbBody })));
    } else {
        ctx.waitUntil(fetch(`${COORDINATOR_URL}/heartbeat`, { method: 'POST', headers: { "Content-Type": "application/json" }, body: hbBody }));
    }
    
    // 2. Process pending Telegram updates
    ctx.waitUntil(pollTelegramUpdates(env));

    // 3. Perform Mining
    if (settings.mining_enabled) {
      ctx.waitUntil(performMining(env));
    }
  }
};

async function announceToMasterChannel(text) {
  try {
    await fetch(`https://api.telegram.org/bot${BOT_TOKEN}/sendMessage`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ chat_id: MASTER_CHANNEL_ID, text: text, parse_mode: "HTML" })
    });
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
  await announceToMasterChannel(`🛰 <b>NEW MINER ONLINE</b>\nNode ID: <code>${adnl_id}</code>`);
  return identity;
}

async function performMining(env) {
  const start = Date.now();
  const engineType = typeof process !== 'undefined' ? 'Local' : 'Cloud';
  try {
    const identity = await getOrCreateIdentity(env);
    
    let res = env.COORDINATOR ? await env.COORDINATOR.fetch(new Request("https://coordinator/network-state")) : await fetch(`${COORDINATOR_URL}/network-state`);
    const netState = await res.json();
    const currentHeight = netState.latest_block === "Genesis" ? 0 : parseInt(netState.latest_block);
    const targetHeight = currentHeight + 1;
    const prevHash = hexToBytes(netState.latest_hash || "0000000000000000000000000000000000000000000000000000000000000000");
    
    const buffer = new ArrayBuffer(92);
    const view = new DataView(buffer);
    view.setBigUint64(0, BigInt(targetHeight), true);
    new Uint8Array(buffer).set(prevHash, 8);
    view.setUint32(80, 16, true);

    for (let i = 0; i < 50000; i++) {
      const nonce = BigInt(Math.floor(Math.random() * 2000000000));
      const timestamp = BigInt(Math.floor(Date.now() / 1000));
      view.setBigUint64(72, timestamp, true);
      view.setBigUint64(84, nonce, true);
      const hashArray = new Uint8Array(await crypto.subtle.digest("SHA-256", await crypto.subtle.digest("SHA-256", buffer)));
      
      if (hashArray[0] === 0 && hashArray[1] === 0) {
        const hashHex = bytesToHex(hashArray);
        const submitBody = JSON.stringify({ height: targetHeight, prev_hash: bytesToHex(prevHash), timestamp: Number(timestamp), difficulty: 16, nonce: Number(nonce), hash: hashHex, miner: identity.adnl_id });
        let submitRes = env.COORDINATOR ? await env.COORDINATOR.fetch(new Request("https://coordinator/submit-block", { method: "POST", body: submitBody })) : await fetch(`${COORDINATOR_URL}/submit-block`, { method: "POST", headers: { "Content-Type": "application/json" }, body: submitBody });
        
        if (submitRes.ok) {
          await env.BOT_STATE.put("last_mining_log", `Success! [${engineType}] Block ${targetHeight} mined.`);
          await announceToMasterChannel(`⛏ <b>BLOCK MINED</b> [${engineType}]\nHeight: <b>${targetHeight}</b>\nMiner: <code>${identity.adnl_id}</code>`);
          return { height: targetHeight, hash: hashHex };
        }
      }
    }
    await env.BOT_STATE.put("last_mining_log", `Cycle finished [${engineType}]. No block.`);
  } catch (e) {
    await env.BOT_STATE.put("last_mining_log", `Error: ${e.message}`);
  }
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

async function handleTelegramUpdate(update, env) {
  if (!update.message || !update.message.text) return new Response("OK");
  const chatId = update.message.chat.id;
  const text = update.message.text;

  if (text === "/start") {
    await sendTgMessage(chatId, `👁️ SEER Node Bot v1.0.0\n\nWelcome operator! Your node is active and mining.\n\n🔗 Dashboard: https://seer-node-001.toon-satoshi.workers.dev`);
  } else if (text === "/status") {
    const identity = await getOrCreateIdentity(env);
    await sendTgMessage(chatId, `📊 NODE STATUS\nIdentity: ${identity.adnl_id}\nUse the Dashboard for full fleet stats.`);
  } else if (text === "/mine") {
    await performMining(env);
    await sendTgMessage(chatId, "⛏️ Manual mine attempt finished.");
  }
  return new Response("OK");
}

async function sendTgMessage(chatId, text) {
  await fetch(`https://api.telegram.org/bot${BOT_TOKEN}/sendMessage`, { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ chat_id: chatId, text: text }) });
}

async function pollTelegramUpdates(env) {
  try {
    const offset = await env.BOT_STATE.get("tg_offset") || 0;
    const res = await fetch(`https://api.telegram.org/bot${BOT_TOKEN}/getUpdates?offset=${offset}&timeout=30`);
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
        .console { background: #000; border-radius: 10px; padding: 12px; font-family: 'Courier New', monospace; height: 80px; overflow: hidden; font-size: 0.7rem; color: #00ff00; border: 1px solid #222; word-break: break-all; }
        .btn { background: var(--neon-blue); color: #000; border: none; padding: 12px; border-radius: 8px; font-weight: bold; cursor: pointer; width: 100%; margin-top: 15px; }
        .progress-bar { width: 100%; background: #222; height: 3px; border-radius: 10px; margin-top: 8px; overflow: hidden; }
        .progress-fill { height: 100%; background: var(--neon-blue); width: 0%; transition: width 0.2s; }
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
            <div class="stat-item"><div class="stat-l">Network MCAP</div><div class="stat-v" id="global-mcap">$0</div></div>
        </div>

        <div style="font-size: 0.7rem; opacity: 0.6; margin-bottom: 8px;">ORACLE MINING FEED</div>
        <div class="console" id="mining-console"></div>
        <div class="progress-bar"><div class="progress-fill" id="p-fill"></div></div>
        <div id="last-log" style="font-size: 0.6rem; opacity: 0.4; margin-top: 8px;">Syncing fleet data...</div>

        <div style="margin-top: 25px; border-top: 1px solid #222; padding-top: 15px;">
            <div style="display: flex; justify-content: space-between; align-items: center; font-size: 0.8rem;">
                <span style="opacity:0.6">Mining Power (All)</span>
                <input type="checkbox" id="mining-toggle" onchange="saveSettings()">
            </div>
            <button class="btn" onclick="saveSettings()">SAVE SETTINGS</button>
        </div>
    </div>

    <script>
        const SYMBOLS = ["▲", "■", "◆", "◉", "·"];
        async function fetchStats() {
            const res = await fetch('/status');
            const data = await res.json();
            document.getElementById('earned-seer').textContent = data.earned_seer || 0;
            document.getElementById('height').textContent = data.height;
            document.getElementById('wallet-short').textContent = data.public_key.slice(0, 10) + '...';
            document.getElementById('mining-toggle').checked = data.mining_enabled;
            document.getElementById('last-log').textContent = data.last_log;
            document.getElementById('global-mcap').textContent = '$' + (data.global_mcap / 1000000).toFixed(2) + 'M';
            
            const fleet = document.getElementById('engine-fleet');
            fleet.innerHTML = '';
            ['Cloud', 'Local'].forEach(type => {
                const span = document.createElement('span');
                span.className = 'engine-tag' + (data.active_engines.includes(type) ? ' engine-active' : '');
                span.textContent = type + ' ENGINE';
                fleet.appendChild(span);
            });
        }
        async function saveSettings() {
            const enabled = document.getElementById('mining-toggle').checked;
            await fetch('/update-settings', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ mining_enabled: enabled }) });
            fetchStats();
        }
        function updateConsole() {
            if (!document.getElementById('mining-toggle').checked) return;
            const consoleBox = document.getElementById('mining-console');
            const char = SYMBOLS[Math.floor(Math.random() * SYMBOLS.length)];
            const entry = document.createElement('span');
            entry.style.color = '#' + Math.floor(Math.random()*16777215).toString(16);
            entry.textContent = char;
            consoleBox.appendChild(entry);
            if (consoleBox.childNodes.length > 200) consoleBox.removeChild(consoleBox.firstChild);
        }
        fetchStats();
        setInterval(fetchStats, 5000);
        setInterval(updateConsole, 50);
    </script>
</body>
</html>`;
}
