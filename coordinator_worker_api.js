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
          market_cap: 1000
        };
        return new Response(JSON.stringify(state), {
          headers: { ...corsHeaders, "Content-Type": "application/json" }
        });
      }

      if (url.pathname === "/submit-block" && request.method === "POST") {
        const block = await request.json();
        
        // 1. Reconstruct 92-byte header
        const buffer = new ArrayBuffer(92);
        const view = new DataView(buffer);
        const height = BigInt(block.height);
        view.setBigUint64(0, height, true);
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
        
        // 3. Oracle-enhanced Tokenomics (derived from tokenomics_oracle.py)
        const prevState = await env.NETWORK_STATE.get("latest", { type: "json" }) || {
          total_supply: 100000000,
          velocity: 0.002,
          sentiment: 0.5,
          gini: 0.35
        };

        const reward = 50;
        const newSupply = prevState.total_supply + reward;
        
        // Dynamic metrics simulation (matching tokenomics_oracle.py formulas)
        const tick = Number(height);
        const sentiment = Math.max(0.1, Math.min(0.9, prevState.sentiment + (Math.random() * 0.02 - 0.01)));
        const velocity = Math.max(0.001, Math.min(0.01, prevState.velocity + (Math.random() * 0.0005 - 0.0002)));
        const gini = Math.max(0.1, Math.min(0.9, prevState.gini + (Math.random() * 0.01 - 0.005)));
        
        // staking_ratio = max(0.08, min(0.92, 0.18 + (gini * 0.72) + sin(tick/137)*0.07 + (sentiment*0.25)))
        const stakingRatio = Math.max(0.08, Math.min(0.92, 0.18 + (gini * 0.72) + Math.sin(tick / 137) * 0.07 + (sentiment * 0.25)));
        
        // market_cap_proxy = (supply * (1.35 + velocity * 1.1 - gini * 0.65)) / 7.5
        const marketCap = (newSupply * (1.35 + velocity * 1.1 - gini * 0.65)) / 7.5;

        const newState = {
          latest_block: Number(height),
          latest_hash: computedHashHex,
          total_supply: newSupply,
          active_nodes: 1,
          network_name: "SEER Mainnet",
          velocity: velocity,
          sentiment: sentiment,
          gini: gini,
          staking_ratio: stakingRatio,
          market_cap: marketCap
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
