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
          latest_hash: "0000000000000000000000000000000000000000000000000000000000000000",
          total_supply: 100000000,
          active_nodes: 1,
          network_name: "SEER Mainnet",
          velocity: 0.002,
          sentiment: 0.5,
          gini: 0.35,
          staking_ratio: 0.15,
          market_cap: 0, // HONEST VALUATION: No liquidity yet
          ton_bridge_active: false
        };
        state.api_version = "ORACLE-v1.2";
        return new Response(JSON.stringify(state), {
          headers: { ...corsHeaders, "Content-Type": "application/json" }
        });
      }

      if (url.pathname === "/submit-block" && request.method === "POST") {
        const block = await request.json();
        
        // 1. Reconstruct 92-byte header
        const buffer = new ArrayBuffer(92);
        const view = new DataView(buffer);
        view.setBigUint64(0, BigInt(block.height), true);
        const prevHash = hexToBytes(block.prev_hash);
        new Uint8Array(buffer).set(prevHash, 8);
        const txRoot = hexToBytes(block.tx_root || "0000000000000000000000000000000000000000000000000000000000000000");
        new Uint8Array(buffer).set(txRoot, 40);
        view.setBigUint64(72, BigInt(block.timestamp), true);
        view.setUint32(80, block.difficulty || 16, true);
        view.setBigUint64(84, BigInt(block.nonce), true);
        
        // 2. Double SHA-256
        const hash1 = await crypto.subtle.digest("SHA-256", buffer);
        const hash2 = await crypto.subtle.digest("SHA-256", hash1);
        const computedHashHex = bytesToHex(new Uint8Array(hash2));
        
        if (computedHashHex !== block.hash) {
          return new Response(JSON.stringify({ error: "Hash mismatch" }), { status: 400, headers: corsHeaders });
        }
        
        const hashBytes = new Uint8Array(hash2);
        if (!verifyDifficulty(hashBytes, block.difficulty || 16)) {
          return new Response(JSON.stringify({ error: "Insufficient difficulty" }), { status: 400, headers: corsHeaders });
        }
        
        // 3. Oracle Tokenomics (Market Cap strictly 0 until TON Bridge / Liquidity exists)
        const prevState = await env.NETWORK_STATE.get("latest", { type: "json" }) || {};
        const prevSupply = Number(prevState.total_supply) || 100000000;
        const prevSentiment = Number(prevState.sentiment) || 0.5;
        const prevVelocity = Number(prevState.velocity) || 0.002;
        const prevGini = Number(prevState.gini) || 0.35;

        const reward = 50;
        const newSupply = prevSupply + reward;
        const tick = Number(block.height) || 0;
        
        const sentiment = Math.max(0.1, Math.min(0.9, prevSentiment + (Math.random() * 0.02 - 0.01)));
        const velocity = Math.max(0.001, Math.min(0.01, prevVelocity + (Math.random() * 0.0005 - 0.0002)));
        const gini = Math.max(0.1, Math.min(0.9, prevGini + (Math.random() * 0.01 - 0.005)));
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
          market_cap: 0, // HONEST VALUATION: Worthless without liquidity pool
          ton_bridge_active: false,
          api_version: "ORACLE-v1.2"
        };
        
        await env.NETWORK_STATE.put("latest", JSON.stringify(newState));
        
        return new Response(JSON.stringify({ status: "Accepted", hash: computedHashHex }), {
          headers: { ...corsHeaders, "Content-Type": "application/json" }
        });
      }

      return new Response("SEER Coordinator Live", { status: 200, headers: corsHeaders });
    } catch (err) {
      return new Response(JSON.stringify({ error: err.message }), {
        status: 500,
        headers: { ...corsHeaders, "Content-Type": "application/json" }
      });
    }
  }
};

function hexToBytes(hex) {
  const bytes = new Uint8Array(hex.length / 2);
  for (let i = 0; i < hex.length; i += 2) {
    bytes[i / 2] = parseInt(hex.substr(i, 2), 16);
  }
  return bytes;
}

function bytesToHex(bytes) {
  return Array.from(bytes).map(b => b.toString(16).padStart(2, '0')).join('');
}

function verifyDifficulty(hash, bits) {
  const fullBytes = Math.floor(bits / 8);
  for (let i = 0; i < fullBytes; i++) {
    if (hash[i] !== 0) return false;
  }
  const remainingBits = bits % 8;
  if (remainingBits > 0) {
    if ((hash[fullBytes] >> (8 - remainingBits)) !== 0) return false;
  }
  return true;
}
