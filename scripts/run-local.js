const fs = require('fs');
const path = require('path');

// --- CONFIGURATION ---
const CONFIG_PATH = path.join(__dirname, '../local_node_config.json');
const STATE_PATH = path.join(__dirname, '../local_node_state.json');

if (!fs.existsSync(CONFIG_PATH)) {
  console.error("❌ Error: local_node_config.json not found!");
  process.exit(1);
}

const config = JSON.parse(fs.readFileSync(CONFIG_PATH, 'utf8'));
const BOT_TOKEN = config.bot_token;

// --- CLOUDFLARE MOCKS ---
const BOT_STATE = {
  get: async (key, options) => {
    if (!fs.existsSync(STATE_PATH)) return null;
    const state = JSON.parse(fs.readFileSync(STATE_PATH, 'utf8') || '{}');
    const val = state[key];
    if (options && options.type === 'json' && typeof val === 'string') return JSON.parse(val);
    return val;
  },
  put: async (key, val) => {
    const state = fs.existsSync(STATE_PATH) ? JSON.parse(fs.readFileSync(STATE_PATH, 'utf8') || '{}') : {};
    state[key] = val;
    fs.writeFileSync(STATE_PATH, JSON.stringify(state, null, 2));
  },
  delete: async (key) => {
    if (!fs.existsSync(STATE_PATH)) return;
    const state = JSON.parse(fs.readFileSync(STATE_PATH, 'utf8') || '{}');
    delete state[key];
    fs.writeFileSync(STATE_PATH, JSON.stringify(state, null, 2));
  },
  list: async (options) => {
    if (!fs.existsSync(STATE_PATH)) return { keys: [] };
    const state = JSON.parse(fs.readFileSync(STATE_PATH, 'utf8') || '{}');
    let keys = Object.keys(state).map(name => ({ name }));
    if (options && options.prefix) {
      keys = keys.filter(k => k.name.startsWith(options.prefix));
    }
    return { keys };
  }
};

// Import worker logic safely
const workerPath = path.join(__dirname, '../bot_node_worker.js');
let workerCode = fs.readFileSync(workerPath, 'utf8');
workerCode = workerCode.replace('export default', 'const worker = ');
workerCode += '\nmodule.exports = worker;';

const tmpWorkerPath = path.join(__dirname, `tmp_worker_${Date.now()}.js`);
fs.writeFileSync(tmpWorkerPath, workerCode);
const worker = require(tmpWorkerPath);
fs.unlinkSync(tmpWorkerPath); 

const env = { BOT_STATE };

console.log(`👁️ SEER LOCAL RUNNER STARTING...`);
console.log(`Node Name: ${config.node_name}`);

async function pollLoop() {
  try {
    const offset = await BOT_STATE.get("tg_offset") || 0;
    const res = await fetch(`https://api.telegram.org/bot${BOT_TOKEN}/getUpdates?offset=${offset}&timeout=30`);
    const data = await res.json();
    
    if (data.ok && data.result.length > 0) {
      let maxId = offset;
      for (const update of data.result) {
        // Manually handle the update through worker.fetch
        const cfRequest = {
            url: `http://localhost:8080/telegram-webhook`,
            method: 'POST',
            json: async () => update,
            headers: new Map([['content-type', 'application/json']])
        };
        await worker.fetch(cfRequest, env, { waitUntil: (p) => p });
        maxId = Math.max(maxId, update.update_id + 1);
      }
      await BOT_STATE.put("tg_offset", maxId.toString());
    }
  } catch (e) {
    console.error("Polling error:", e.message);
  }
  setTimeout(pollLoop, 1000);
}

async function miningLoop() {
  console.log("⛏️ Starting mining cycle...");
  const settings = await BOT_STATE.get("settings", { type: "json" }) || { mining_enabled: true };
  if (settings.mining_enabled) {
    await worker.scheduled(null, env, { waitUntil: (p) => p });
  }
  setTimeout(miningLoop, 60000);
}

const http = require('http');
const server = http.createServer(async (req, res) => {
    const url = new URL(req.url, `http://${req.headers.host}`);
    
    let body = '';
    req.on('data', chunk => body += chunk);
    req.on('end', async () => {
        const cfRequest = {
            url: url.toString(),
            method: req.method,
            json: async () => JSON.parse(body || '{}'),
            text: async () => body,
            headers: new Map(Object.entries(req.headers))
        };

        try {
            const response = await worker.fetch(cfRequest, env, { waitUntil: (p) => p });
            res.writeHead(response.status || 200, { 'Content-Type': 'application/json', 'Access-Control-Allow-Origin': '*' });
            res.end(await response.text());
        } catch (e) {
            res.writeHead(500);
            res.end(JSON.stringify({ error: e.message }));
        }
    });
});

server.listen(8080, () => {
    console.log("🌐 Local Dashboard & API live at http://localhost:8080");
    pollLoop();
    miningLoop();
});
