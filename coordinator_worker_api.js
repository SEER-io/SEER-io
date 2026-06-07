const BOT_TOKEN = "8951904080:AAFbh5R5F1bZz-am9BF0F2drcqxMuTQI6RM";

export default {
  async fetch(request, env, ctx) {
    const url = new URL(request.url);

    const corsHeaders = {
      "Access-Control-Allow-Origin": "*",
      "Access-Control-Allow-Methods": "GET, POST, OPTIONS",
      "Access-Control-Allow-Headers": "Content-Type",
    };

    if (request.method === "OPTIONS") {
      return new Response(null, { headers: corsHeaders });
    }

    try {
      if (url.pathname === "/network-state" && request.method === "GET") {
        const state = await env.NETWORK_STATE.get("latest", { type: "json" }) || {
          latest_block: 0,
          total_supply: 100000000,
          active_nodes: 1,
          network_name: "SEER Mainnet"
        };
        
        // Count Active Engines
        const engineList = await env.NETWORK_STATE.list({ prefix: "engine:" });
        let cloudCount = 0;
        let localCount = 0;
        for (const key of engineList.keys) {
          if (key.name.endsWith(":Cloud")) cloudCount++;
          if (key.name.endsWith(":Local")) localCount++;
        }
        state.cloud_engines = cloudCount;
        state.local_engines = localCount;

        // Swarm Count
        const swarmList = await env.NETWORK_STATE.list({ prefix: "swarm:" });
        state.swarm_bots = swarmList.keys.length;

        // Get Recent Blocks
        state.recent_blocks = await env.NETWORK_STATE.get("recent_blocks", { type: "json" }) || [];
        
        // Calculate % Mined
        const mineableRewards = 900000000;
        state.percent_mined = ((state.total_supply - 100000000) / mineableRewards) * 100;

        return new Response(JSON.stringify(state), { headers: { ...corsHeaders, "Content-Type": "application/json" } });
      }

      if (url.pathname === "/register-bot" && request.method === "POST") {
        const { bot_username } = await request.json();
        const username = bot_username.replace("@", "").trim();

        // 1. Verify Bot on Telegram
        // We use our Genesis Bot token to check the user info
        const tgRes = await fetch(`https://api.telegram.org/bot${BOT_TOKEN}/getChat?chat_id=@${username}`);
        const tgData = await tgRes.json();

        if (!tgData.ok) {
          return new Response(JSON.stringify({ 
            error: "Bot not found on Telegram.",
            fix: "Ensure your bot is public and you have the correct @username."
          }), { status: 400, headers: corsHeaders });
        }

        // 2. Add to Swarm
        await env.NETWORK_STATE.put(`swarm:${username}`, JSON.stringify({
          username,
          registered_at: Date.now()
        }));

        // 3. Genesis Bot Announces
        await fetch(`https://api.telegram.org/bot${BOT_TOKEN}/sendMessage`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            chat_id: "-1003997728534", // Master Channel
            text: `🛰 <b>SWARM NODE REGISTERED</b>\nBot: @${username}\nStatus: ACTIVE\n\n<i>Note: New bot added to the global signal registry.</i>`,
            parse_mode: "HTML"
          })
        });

        return new Response(JSON.stringify({ success: true, message: "Bot added to the swarm registry." }), { headers: corsHeaders });
      }

      if (url.pathname === "/heartbeat" && request.method === "POST") {
        const data = await request.json();
        await env.NETWORK_STATE.put(`engine:${data.miner_id}:${data.engine_type}`, Date.now().toString(), { expirationTtl: 300 });
        return new Response(JSON.stringify({ success: true }), { headers: corsHeaders });
      }

      if (url.pathname === "/engines" && request.method === "GET") {
        const miner_id = url.searchParams.get("miner_id");
        const list = await env.NETWORK_STATE.list({ prefix: `engine:${miner_id}:` });
        return new Response(JSON.stringify({ engines: list.keys.map(k => k.name.split(':').pop()) }), { headers: { ...corsHeaders, "Content-Type": "application/json" } });
      }

      if (url.pathname === "/miner-stats" && request.method === "GET") {
        const miner_id = url.searchParams.get("id");
        const blocks = await env.NETWORK_STATE.get(`miner:${miner_id}:blocks`) || "0";
        return new Response(JSON.stringify({ blocks: parseInt(blocks) }), { headers: { ...corsHeaders, "Content-Type": "application/json" } });
      }

      if (url.pathname === "/submit-block" && request.method === "POST") {
        const block = await request.json();
        const buffer = new ArrayBuffer(92);
        const view = new DataView(buffer);
        view.setBigUint64(0, BigInt(block.height), true);
        new Uint8Array(buffer).set(hexToBytes(block.prev_hash), 8);
        new Uint8Array(buffer).set(hexToBytes(block.tx_root || "0000000000000000000000000000000000000000000000000000000000000000"), 40);
        view.setBigUint64(72, BigInt(block.timestamp), true);
        view.setUint32(80, block.difficulty || 16, true);
        view.setBigUint64(84, BigInt(block.nonce), true);
        
        const hashArray = new Uint8Array(await crypto.subtle.digest("SHA-256", await crypto.subtle.digest("SHA-256", buffer)));
        const computedHashHex = bytesToHex(hashArray);
        
        if (computedHashHex !== block.hash) return new Response(JSON.stringify({ error: "Hash mismatch" }), { status: 400, headers: corsHeaders });
        if (!verifyDifficulty(hashArray, block.difficulty || 16)) return new Response(JSON.stringify({ error: "Insufficient difficulty" }), { status: 400, headers: corsHeaders });
        
        const prevState = await env.NETWORK_STATE.get("latest", { type: "json" }) || { total_supply: 100000000, velocity: 0.002, sentiment: 0.5, gini: 0.35 };
        const newSupply = Number(prevState.total_supply) + 50;
        const tick = Number(block.height);
        
        const sentiment = Math.max(0.1, Math.min(0.9, (prevState.sentiment || 0.5) + (Math.random() * 0.02 - 0.01)));
        const velocity = Math.max(0.001, Math.min(0.01, (prevState.velocity || 0.002) + (Math.random() * 0.0005 - 0.0002)));
        const gini = Math.max(0.1, Math.min(0.9, (prevState.gini || 0.35) + (Math.random() * 0.01 - 0.005)));
        const stakingRatio = Math.max(0.08, Math.min(0.92, 0.18 + (gini * 0.72) + Math.sin(tick / 137) * 0.07 + (sentiment * 0.25)));

        const newState = {
          latest_block: tick,
          latest_hash: computedHashHex,
          total_supply: newSupply,
          active_nodes: 1,
          network_name: "SEER Mainnet",
          velocity: Number(velocity.toFixed(6)),
          sentiment: Number(sentiment.toFixed(6)),
          gini: Number(gini.toFixed(6)),
          staking_ratio: Number(stakingRatio.toFixed(6)),
          market_cap: 0,
          ton_bridge_active: false,
          api_version: "ORACLE-v1.4"
        };
        
        await env.NETWORK_STATE.put("latest", JSON.stringify(newState));
        await env.NETWORK_STATE.put(`miner:${block.miner}:blocks`, (parseInt(await env.NETWORK_STATE.get(`miner:${block.miner}:blocks`) || 0) + 1).toString());

        const recentBlocks = await env.NETWORK_STATE.get("recent_blocks", { type: "json" }) || [];
        recentBlocks.unshift({ height: tick, hash: computedHashHex, miner: block.miner, timestamp: block.timestamp, nonce: block.nonce, difficulty: block.difficulty || 16 });
        if (recentBlocks.length > 10) recentBlocks.pop();
        await env.NETWORK_STATE.put("recent_blocks", JSON.stringify(recentBlocks));

        return new Response(JSON.stringify({ status: "Accepted", hash: computedHashHex }), { headers: { ...corsHeaders, "Content-Type": "application/json" } });
      }

      return new Response("SEER Coordinator Live", { status: 200, headers: corsHeaders });
    } catch (err) {
      return new Response(JSON.stringify({ error: err.message }), { status: 500, headers: { ...corsHeaders, "Content-Type": "application/json" } });
    }
  }
};

function hexToBytes(hex) {
  const bytes = new Uint8Array(hex.length / 2);
  for (let i = 0; i < hex.length; i += 2) bytes[i / 2] = parseInt(hex.substr(i, 2), 16);
  return bytes;
}

function bytesToHex(bytes) {
  return Array.from(bytes).map(b => b.toString(16).padStart(2, '0')).join('');
}

function verifyDifficulty(hash, bits) {
  const fullBytes = Math.floor(bits / 8);
  for (let i = 0; i < fullBytes; i++) if (hash[i] !== 0) return false;
  const remainingBits = bits % 8;
  if (remainingBits > 0 && (hash[fullBytes] >> (8 - remainingBits)) !== 0) return false;
  return true;
}
