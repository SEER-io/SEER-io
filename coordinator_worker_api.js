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
          network_name: "SEER Mainnet"
        };
        return new Response(JSON.stringify(state), {
          headers: { ...corsHeaders, "Content-Type": "application/json" }
        });
      }

      if (url.pathname === "/submit-block" && request.method === "POST") {
        const block = await request.json();
        
        // --- PRIORITY 1: CRYPTOGRAPHIC VERIFICATION ---
        
        // 1. Reconstruct 92-byte header
        const buffer = new ArrayBuffer(92);
        const view = new DataView(buffer);
        
        // height (8 bytes, LE)
        const height = BigInt(block.height);
        view.setBigUint64(0, height, true);
        
        // prev_hash (32 bytes)
        const prevHash = hexToBytes(block.prev_hash);
        new Uint8Array(buffer).set(prevHash, 8);
        
        // tx_root (32 bytes)
        const txRoot = hexToBytes(block.tx_root || "0000000000000000000000000000000000000000000000000000000000000000");
        new Uint8Array(buffer).set(txRoot, 40);
        
        // timestamp (8 bytes, LE)
        view.setBigUint64(72, BigInt(block.timestamp), true);
        
        // difficulty (4 bytes, LE)
        view.setUint32(80, block.difficulty || 16, true);
        
        // nonce (8 bytes, LE)
        view.setBigUint64(84, BigInt(block.nonce), true);
        
        // 2. Compute SHA256d (Double SHA-256)
        const hash1 = await crypto.subtle.digest("SHA-256", buffer);
        const hash2 = await crypto.subtle.digest("SHA-256", hash1);
        const computedHashHex = bytesToHex(new Uint8Array(hash2));
        
        // 3. Verify Hash matches submission
        if (computedHashHex !== block.hash) {
          return new Response(JSON.stringify({ error: "Hash mismatch" }), { status: 400, headers: corsHeaders });
        }
        
        // 4. Verify Difficulty (16-bit leading zero check)
        const hashBytes = new Uint8Array(hash2);
        const targetDifficulty = block.difficulty || 16;
        if (!verifyDifficulty(hashBytes, targetDifficulty)) {
          return new Response(JSON.stringify({ error: "Insufficient difficulty" }), { status: 400, headers: corsHeaders });
        }
        
        // 5. Update global state
        const reward = 50;
        const newState = {
          latest_block: Number(height),
          latest_hash: computedHashHex,
          total_supply: 100000000 + (Number(height) * reward),
          active_nodes: 1,
          network_name: "SEER Mainnet"
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
