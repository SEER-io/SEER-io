/**
 * SEER Network Coordinator - Cloudflare Worker
 */

export default {
  async fetch(request, env) {
    const url = new URL(request.url);

    // 1. CORS Headers
    const corsHeaders = {
      "Access-Control-Allow-Origin": "*",
      "Access-Control-Allow-Methods": "GET, POST, OPTIONS",
      "Access-Control-Allow-Headers": "Content-Type",
    };

    if (request.method === "OPTIONS") {
      return new Response(null, { headers: corsHeaders });
    }

    try {
      // 2. Routing
      if (url.pathname === "/network-state" && request.method === "GET") {
        return await handleNetworkState(env, corsHeaders);
      }

      if (url.pathname === "/register" && request.method === "POST") {
        const body = await request.json();
        return await handleRegister(body, env, corsHeaders);
      }

      // 3. Telegram Webhook (Wire Format: SEER/{KIND}/{SEQ}/{SENDER_ID}/{PAYLOAD_HEX})
      if (request.method === "POST") {
        const payload = await request.json();
        if (payload.message && payload.message.text) {
          return await handleWireMessage(payload.message.text, env, corsHeaders);
        }
      }

      return new Response("Not Found", { status: 404, headers: corsHeaders });
    } catch (err) {
      return new Response(JSON.stringify({ error: err.message }), {
        status: 500,
        headers: { ...corsHeaders, "Content-Type": "application/json" }
      });
    }
  }
};

/**
 * Parses wire format: SEER/{KIND}/{SEQ}/{SENDER_ID}/{PAYLOAD_HEX}
 */
async function handleWireMessage(text, env, headers) {
  if (!text.startsWith("SEER/")) {
    return new Response("OK", { headers });
  }

  const parts = text.split("/");
  if (parts.length < 5) return new Response("Malformed Wire Format", { status: 400, headers });

  const [_, kind, seq, sender_id, payload_hex] = parts;

  switch (kind) {
    case "STATUS":
      await env.NETWORK_STATE.put(`node:${sender_id}:last_seen`, Date.now());
      await env.NETWORK_STATE.put(`node:${sender_id}:tip`, payload_hex);
      break;
    case "BLOCK":
      await env.NETWORK_STATE.put("latest_block", payload_hex);
      await announceToChannel(`New Block Found by ${sender_id}: ${payload_hex}`, env);
      break;
    // Add other kinds as needed
  }

  return new Response("OK", { headers });
}

async function handleRegister(body, env, headers) {
  const { bot_token, node_id } = body;

  if (!bot_token || !node_id) {
    return new Response(JSON.stringify({ error: "Missing token or ID" }), { status: 400, headers });
  }

  // Derive ID to verify (Security check)
  const expectedId = await deriveNodeId(bot_token);
  if (node_id !== expectedId) {
    return new Response(JSON.stringify({ error: "Invalid Node ID derivation" }), { status: 403, headers });
  }

  // Store in KV
  await env.NETWORK_STATE.put(`node:${node_id}:registered`, "true");
  await env.NETWORK_STATE.put(`node:${node_id}:last_seen`, Date.now());

  // Store in D1
  await env.SEER_DB.prepare(
    "INSERT OR REPLACE INTO nodes (id, last_seen, registered_at) VALUES (?, ?, ?)"
  ).bind(node_id, Date.now(), Date.now()).run();

  // Announce
  await announceToChannel(`🚀 New Node Registered: @seer_${node_id.substring(0, 6)}_bot`, env);

  return new Response(JSON.stringify({ status: "Registered", node_id }), {
    headers: { ...headers, "Content-Type": "application/json" }
  });
}

async function handleNetworkState(env, headers) {
  const latestBlock = await env.NETWORK_STATE.get("latest_block") || "Genesis";
  const nodes = await env.SEER_DB.prepare("SELECT COUNT(*) as count FROM nodes").first("count");
  
  // Dummy supply calculation
  const supply = 100000000; 

  return new Response(JSON.stringify({
    latest_block: latestBlock,
    total_supply: supply,
    active_nodes: nodes,
    network_name: "SEER Mainnet"
  }), {
    headers: { ...headers, "Content-Type": "application/json" }
  });
}

async function announceToChannel(text, env) {
  const token = env.BOT_TOKEN; // Ideally provided via secret env var
  const chatId = env.MASTER_CHANNEL_ID;
  const url = `https://api.telegram.org/bot${token}/sendMessage`;

  await fetch(url, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ chat_id: chatId, text })
  });
}

async function deriveNodeId(token) {
  const msgUint8 = new TextEncoder().encode(token);
  const hashBuffer = await crypto.subtle.digest("SHA-256", msgUint8);
  const hashArray = Array.from(new Uint8Array(hashBuffer));
  const hashHex = hashArray.map(b => b.toString(16).padStart(2, "0")).join("");
  return hashHex.substring(0, 12);
}
