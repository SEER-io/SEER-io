addEventListener('fetch', event => {
  event.respondWith(handleRequest(event.request))
})

async function handleRequest(request) {
  const html = `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>SEER Network - Global Explorer</title>
    <style>
        :root { --neon-green: #00ff41; --dark-bg: #0d0208; }
        body { background: var(--dark-bg); color: var(--neon-green); font-family: 'Courier New', Courier, monospace; margin: 0; padding: 20px; }
        .container { max-width: 900px; margin: 0 auto; border: 2px solid var(--neon-green); padding: 20px; box-shadow: 0 0 15px var(--neon-green); }
        h1 { text-align: center; text-transform: uppercase; letter-spacing: 5px; border-bottom: 2px solid var(--neon-green); padding-bottom: 20px; }
        .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 20px; margin-top: 20px; }
        .stat-box { border: 1px solid var(--neon-green); padding: 15px; text-align: center; }
        .stat-label { font-size: 0.8rem; text-transform: uppercase; opacity: 0.7; display: block; margin-bottom: 5px; }
        .stat-value { font-size: 1.2rem; font-weight: bold; }
        .footer { margin-top: 30px; text-align: center; font-size: 0.7rem; opacity: 0.5; }
        .live-indicator { display: inline-block; width: 10px; height: 10px; background: red; border-radius: 50%; margin-right: 10px; animation: blink 1s infinite; }
        .alert-pill { background: #330000; color: #ff0000; border: 1px solid #ff0000; padding: 2px 8px; border-radius: 4px; font-size: 0.6rem; vertical-align: middle; margin-left: 5px; }
        @keyframes blink { 0% { opacity: 1; } 50% { opacity: 0; } 100% { opacity: 1; } }
    </style>
</head>
<body>
    <div class="container">
        <h1>👁️ SEER EXPLORER</h1>
        <p style="text-align: center;"><span class="live-indicator"></span>ORACLE-ENHANCED LIVE FEED</p>
        
        <div class="grid" id="dashboard">
            <div class="stat-box">
                <span class="stat-label">Latest Block</span>
                <span class="stat-value" id="latest-block">SCANNING...</span>
            </div>
            <div class="stat-box">
                <span class="stat-label">Active Nodes</span>
                <span class="stat-value" id="active-nodes">SCANNING...</span>
            </div>
            <div class="stat-box">
                <span class="stat-label">Mined Supply</span>
                <span class="stat-value" id="mined-supply">0</span>
            </div>
            <div class="stat-box">
                <span class="stat-label">Total Supply</span>
                <span class="stat-value" id="total-supply">SCANNING...</span>
            </div>
            <div class="stat-box">
                <span class="stat-label">Market Cap Proxy <span class="alert-pill">NO LIQUIDITY</span></span>
                <span class="stat-value" id="mcap" style="color: #ff0000;">$0.00M</span>
            </div>
            <div class="stat-box" style="border-style: dashed; opacity: 0.8;">
                <span class="stat-label">TON Bridge</span>
                <span class="stat-value" id="bridge-status">OFFLINE</span>
            </div>
            <div class="stat-box">
                <span class="stat-label">Network Velocity</span>
                <span class="stat-value" id="velocity">0.000</span>
            </div>
            <div class="stat-box">
                <span class="stat-label">Gini Index</span>
                <span class="stat-value" id="gini">0.00000</span>
            </div>
        </div>

        <div class="footer">
            SEER NETWORK v1.0.0 | COORDINATOR: seer-coordinator.toon-satoshi.workers.dev
        </div>
    </div>

    <script>
        async function updateStats() {
            try {
                const res = await fetch('https://seer-coordinator.toon-satoshi.workers.dev/network-state?t=' + Date.now());
                const data = await res.json();
                
                document.getElementById('latest-block').textContent = data.latest_block;
                document.getElementById('active-nodes').textContent = data.active_nodes || '1';
                
                const total = Number(data.total_supply) || 100000000;
                const mined = total - 100000000;
                
                document.getElementById('total-supply').textContent = (total / 1000000).toFixed(4) + 'M';
                document.getElementById('mined-supply').textContent = mined.toLocaleString();
                
                // New Oracle Metrics (Zeroed out for no liquidity)
                document.getElementById('velocity').textContent = (data.velocity || 0.002).toFixed(3);
                document.getElementById('gini').textContent = (data.gini || 0.35).toFixed(5);
                document.getElementById('mcap').textContent = '$0.00M';
                document.getElementById('bridge-status').textContent = data.ton_bridge_active ? "ACTIVE" : "OFFLINE";

            } catch (e) {
                console.error('Update failed:', e);
                if (document.getElementById('latest-block').textContent === 'SCANNING...') {
                  document.getElementById('latest-block').textContent = 'SYNC ERROR';
                }
            }
        }
        updateStats();
        setInterval(updateStats, 5000);
    </script>
</body>
</html>`;
    
  return new Response(html, {
    headers: { "Content-Type": "text/html" }
  })
}
