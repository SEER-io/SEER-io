const BOT_TOKEN = "8951904080:AAFbh5R5F1bZz-am9BF0F2drcqxMuTQI6RM";
const COORDINATOR_URL = "https://seer-coordinator.toon-satoshi.workers.dev";

export default {
  async fetch(request, env, ctx) {
    const url = new URL(request.url);

    // 1. Handle Telegram Webhooks
    if (request.method === "POST" && url.pathname === `/bot${BOT_TOKEN}`) {
      const update = await request.json();
      return handleTelegramUpdate(update, env);
    }

    // 2. Handle API Status
    if (url.pathname === "/status") {
      const state = await env.BOT_STATE.get("node_state", { type: "json" }) || { height: 0, blocks_mined: 0 };
      const settings = await env.BOT_STATE.get("settings", { type: "json" }) || { mining_enabled: true, node_name: "SEER Node 001" };
      const lastLog = await env.BOT_STATE.get("last_mining_log") || "No logs yet.";
      const nodeID = await getNodeID(env);
      return new Response(JSON.stringify({ ...state, ...settings, node_id: nodeID, last_log: lastLog }), { 
        headers: { "Content-Type": "application/json", "Access-Control-Allow-Origin": "*" } 
      });
    }

    // 3. Handle Settings Update
    if (request.method === "POST" && url.pathname === "/update-settings") {
      const newSettings = await request.json();
      const currentSettings = await env.BOT_STATE.get("settings", { type: "json" }) || { mining_enabled: true, node_name: "SEER Node 001" };
      const updated = { ...currentSettings, ...newSettings };
      await env.BOT_STATE.put("settings", JSON.stringify(updated));
      return new Response(JSON.stringify({ success: true }), { headers: { "Content-Type": "application/json", "Access-Control-Allow-Origin": "*" } });
    }

    // 4. Serve Mini App Dashboard
    if (url.pathname === "/" || url.pathname === "/index.html") {
      return new Response(generateDashboardHTML(), { headers: { "Content-Type": "text/html" } });
    }

    return new Response("SEER Bot Node Live");
  },

  async scheduled(event, env, ctx) {
    const settings = await env.BOT_STATE.get("settings", { type: "json" }) || { mining_enabled: true };
    if (settings.mining_enabled) {
      ctx.waitUntil(performMining(env));
    }
  }
};

function generateDashboardHTML() {
  return `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>SEER Node - Mini App</title>
    <style>
        :root { --neon-blue: #00f2ff; --dark-bg: #050505; --panel-bg: #111; }
        body { background: var(--dark-bg); color: var(--neon-blue); font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif; margin: 0; padding: 20px; display: flex; flex-direction: column; align-items: center; }
        .card { background: var(--panel-bg); border: 1px solid var(--neon-blue); padding: 25px; border-radius: 12px; width: 100%; max-width: 400px; box-shadow: 0 0 20px rgba(0, 242, 255, 0.2); }
        h1 { margin: 0 0 20px 0; font-size: 1.2rem; text-transform: uppercase; letter-spacing: 2px; border-bottom: 1px solid #333; padding-bottom: 10px; }
        .stat-row { display: flex; justify-content: space-between; margin-bottom: 15px; font-size: 0.9rem; }
        .stat-label { opacity: 0.6; }
        .stat-value { font-weight: bold; }
        .settings { margin-top: 30px; border-top: 1px solid #333; padding-top: 20px; }
        .toggle-container { display: flex; align-items: center; justify-content: space-between; margin-bottom: 20px; }
        input[type="text"] { background: #000; border: 1px solid #333; color: var(--neon-blue); padding: 8px; border-radius: 4px; width: 100%; margin-top: 10px; }
        .btn { background: var(--neon-blue); color: #000; border: none; padding: 10px 20px; border-radius: 6px; font-weight: bold; cursor: pointer; width: 100%; margin-top: 15px; }
        .btn:hover { opacity: 0.8; }
        .status-dot { height: 10px; width: 10px; background-color: #00f2ff; border-radius: 50%; display: inline-block; margin-right: 8px; box-shadow: 0 0 10px var(--neon-blue); }
        .mining-active { animation: pulse 2s infinite; }
        .console { background: #000; border: 1px solid #222; border-radius: 8px; padding: 15px; margin-top: 20px; font-family: 'Courier New', monospace; height: 120px; overflow-y: hidden; font-size: 0.7rem; color: #00ff00; opacity: 0.8; }
        .log-entry { margin-bottom: 4px; white-space: nowrap; }
        .progress-container { width: 100%; background: #222; border-radius: 10px; height: 4px; margin-top: 10px; overflow: hidden; }
        .progress-bar { width: 0%; height: 100%; background: var(--neon-blue); box-shadow: 0 0 10px var(--neon-blue); transition: width 0.1s; }
        @keyframes pulse { 0% { transform: scale(0.95); box-shadow: 0 0 0 0 rgba(0, 242, 255, 0.7); } 70% { transform: scale(1); box-shadow: 0 0 0 10px rgba(0, 242, 255, 0); } 100% { transform: scale(0.95); box-shadow: 0 0 0 0 rgba(0, 242, 255, 0); } }
    </style>
</head>
<body>
    <div class="card">
        <h1>👁️ <span id="node-name-display">LOADING...</span></h1>
        
        <div class="stat-row">
            <span class="stat-label">Node ID</span>
            <span class="stat-value" id="node-id">...</span>
        </div>
        <div class="stat-row">
            <span class="stat-label">Network Height</span>
            <span class="stat-value" id="height">0</span>
        </div>
        <div class="stat-row">
            <span class="stat-label">Blocks Mined</span>
            <span class="stat-value" id="blocks-mined">0</span>
        </div>
        <div class="stat-row">
            <span class="stat-label">Status</span>
            <span class="stat-value"><span id="mining-dot" class="status-dot"></span><span id="status-text">SYNCING</span></span>
        </div>

        <div class="console" id="mining-console">
            <div class="log-entry">> Initialising SEER Core...</div>
            <div class="log-entry">> Awaiting network sync...</div>
        </div>
        <div id="last-log-display" style="font-size: 0.6rem; opacity: 0.5; margin-top: 5px; width: 100%; text-align: left;">
            Last: LOADING...
        </div>
        <div class="progress-container">
            <div class="progress-bar" id="mining-progress"></div>
        </div>

        <div class="settings">
            <div class="toggle-container">
                <span>Mining Power</span>
                <input type="checkbox" id="mining-toggle" style="width: 20px; height: 20px;">
            </div>
            
            <label class="stat-label">Node Name</label>
            <input type="text" id="node-name-input" placeholder="Enter node name">
            
            <button class="btn" onclick="saveSettings()">SAVE SETTINGS</button>
        </div>
    </div>

    <script>
        async function fetchStats() {
            const res = await fetch('/status');
            const data = await res.json();
            
            document.getElementById('node-name-display').textContent = data.node_name;
            document.getElementById('node-name-input').value = data.node_name;
            document.getElementById('node-id').textContent = data.node_id.slice(0, 12);
            document.getElementById('height').textContent = data.height;
            document.getElementById('blocks-mined').textContent = data.blocks_mined;
            document.getElementById('mining-toggle').checked = data.mining_enabled;
            document.getElementById('last-log-display').textContent = "Last: " + data.last_log;
            
            const statusText = data.mining_enabled ? "MINING" : "IDLE";
            document.getElementById('status-text').textContent = statusText;
            
            if(data.mining_enabled) {
                document.getElementById('mining-dot').classList.add('mining-active');
            } else {
                document.getElementById('mining-dot').classList.remove('mining-active');
            }
        }

        async function saveSettings() {
            const name = document.getElementById('node-name-input').value;
            const enabled = document.getElementById('mining-toggle').checked;
            
            await fetch('/update-settings', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ node_name: name, mining_enabled: enabled })
            });
            
            alert('Settings Saved!');
            fetchStats();
        }

        let consoleTimer;
        function updateConsole() {
            const consoleBox = document.getElementById('mining-console');
            const toggle = document.getElementById('mining-toggle');
            const progressBar = document.getElementById('mining-progress');
            
            if (toggle.checked) {
                // Generate a fake hash for visual effect
                const hash = Array.from(crypto.getRandomValues(new Uint8Array(16))).map(b => b.toString(16).padStart(2, '0')).join('');
                const entry = document.createElement('div');
                entry.className = 'log-entry';
                entry.textContent = '> HASH: ' + hash.slice(0, 24) + '... (nonce: ' + Math.floor(Math.random() * 10000) + ')';
                consoleBox.appendChild(entry);
                
                // Keep the last 6 entries
                while (consoleBox.childNodes.length > 6) {
                    consoleBox.removeChild(consoleBox.firstChild);
                }
                
                // Animate progress bar
                let progress = parseFloat(progressBar.style.width || "0");
                progress = (progress + (Math.random() * 10)) % 101;
                progressBar.style.width = progress + "%";
            } else {
                progressBar.style.width = "0%";
            }
        }

        fetchStats();
        setInterval(fetchStats, 10000);
        setInterval(updateConsole, 250);
    </script>
</body>
</html>`;
}

async function handleTelegramUpdate(update, env) {
  if (!update.message || !update.message.text) return new Response("OK");

  const chatId = update.message.chat.id;
  const text = update.message.text;

  if (text === "/start") {
    const url = "https://seer-node-001.toon-satoshi.workers.dev";
    await sendTgMessage(chatId, `👁️ SEER Node Bot v1.0.0\n\nWelcome operator! Your node is active and mining in the background.\n\n🛠️ **DASHBOARD SETUP**:\n1. Click the "Dashboard" button in the menu (bottom left).\n2. Save this as a Mini App for easy access.\n\n🔗 Dashboard Link: ${url}\n\nAvailable commands:\n/status - View node performance\n/mine - Trigger manual mining\n/mining_on - Enable background mining\n/mining_off - Disable background mining`);
  } else if (text === "/mining_on") {
    await env.BOT_STATE.put("settings", JSON.stringify({ mining_enabled: true, node_name: (await env.BOT_STATE.get("settings", {type: "json"}) || {}).node_name || "SEER Node 001" }));
    await sendTgMessage(chatId, "✅ Background mining ENABLED.");
  } else if (text === "/mining_off") {
    const settings = await env.BOT_STATE.get("settings", {type: "json"}) || { node_name: "SEER Node 001" };
    await env.BOT_STATE.put("settings", JSON.stringify({ ...settings, mining_enabled: false }));
    await sendTgMessage(chatId, "🛑 Background mining DISABLED.");
  } else if (text === "/status") {
    const state = await env.BOT_STATE.get("node_state", { type: "json" }) || { height: 0, blocks_mined: 0 };
    const settings = await env.BOT_STATE.get("settings", { type: "json" }) || { mining_enabled: true, node_name: "SEER Node 001" };
    await sendTgMessage(chatId, `📊 ${settings.node_name} STATUS\nHeight: ${state.height}\nBlocks Mined: ${state.blocks_mined}\nMining: ${settings.mining_enabled ? 'ON' : 'OFF'}\nNode ID: ${await getNodeID(env)}`);
  } else if (text === "/mine") {
    await sendTgMessage(chatId, "⛏️ Manual mining attempt started...");
    const result = await performMining(env);
    if (result) {
      await sendTgMessage(chatId, `✅ Block mined! Height: ${result.height}\nHash: ${result.hash.slice(0, 16)}...`);
    } else {
      await sendTgMessage(chatId, "❌ No block found in this cycle.");
    }
  }

  return new Response("OK");
}

async function sendTgMessage(chatId, text) {
  await fetch(`https://api.telegram.org/bot${BOT_TOKEN}/sendMessage`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ chat_id: chatId, text: text })
  });
}

async function getNodeID(env) {
  let id = await env.BOT_STATE.get("node_id");
  if (!id) {
    id = Array.from(crypto.getRandomValues(new Uint8Array(12))).map(b => b.toString(16).padStart(2, '0')).join('');
    await env.BOT_STATE.put("node_id", id);
  }
  return id;
}

async function performMining(env) {
  const start = Date.now();
  try {
    const res = await fetch(`${COORDINATOR_URL}/network-state`);
    const netState = await res.json();
    
    const currentHeight = netState.latest_block === "Genesis" ? 0 : parseInt(netState.latest_block);
    const targetHeight = currentHeight + 1;
    const prevHash = netState.latest_hash;
    const nodeID = await getNodeID(env);
    
    // Attempt 100,000 nonces (increased limit)
    // We use a local loop to minimize TextEncoder overhead
    const encoder = new TextEncoder();
    const baseData = `${targetHeight}${prevHash}${nodeID}`;
    
    for (let i = 0; i < 100000; i++) {
      const nonce = Math.floor(Math.random() * 2000000000);
      const timestamp = Math.floor(Date.now() / 1000);
      const data = encoder.encode(baseData + timestamp + nonce);
      const hashBuffer = await crypto.subtle.digest("SHA-256", data);
      const hashArray = new Uint8Array(hashBuffer);
      
      // 16-bit difficulty (2 bytes zero)
      if (hashArray[0] === 0 && hashArray[1] === 0) {
        const hashHex = Array.from(hashArray).map(b => b.toString(16).padStart(2, '0')).join('');
        
        const submitRes = await fetch(`${COORDINATOR_URL}/submit-block`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            height: targetHeight,
            hash: hashHex,
            nonce: nonce,
            miner: nodeID
          })
        });
        
        if (submitRes.ok) {
          let localState = await env.BOT_STATE.get("node_state", { type: "json" }) || { height: 0, blocks_mined: 0 };
          localState.height = targetHeight;
          localState.blocks_mined++;
          await env.BOT_STATE.put("node_state", JSON.stringify(localState));
          await env.BOT_STATE.put("last_mining_log", `Success! Block ${targetHeight} mined in ${Date.now() - start}ms`);
          return { height: targetHeight, hash: hashHex };
        }
      }
    }
    await env.BOT_STATE.put("last_mining_log", `Cycle finished. 100k hashes tried in ${Date.now() - start}ms. No block found.`);
  } catch (e) {
    await env.BOT_STATE.put("last_mining_log", `Error: ${e.message}`);
  }
  return null;
}
