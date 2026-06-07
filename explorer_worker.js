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
        :root { --neon-green: #00ff41; --dark-bg: #0d0208; --panel-bg: #1a1a1a; }
        body { background: var(--dark-bg); color: var(--neon-green); font-family: 'Courier New', Courier, monospace; margin: 0; padding: 20px; }
        .container { max-width: 1000px; margin: 0 auto; border: 2px solid var(--neon-green); padding: 20px; box-shadow: 0 0 15px var(--neon-green); }
        h1 { text-align: center; text-transform: uppercase; letter-spacing: 5px; border-bottom: 2px solid var(--neon-green); padding-bottom: 20px; margin-bottom: 30px; }
        .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 15px; margin-bottom: 30px; }
        .stat-box { border: 1px solid var(--neon-green); padding: 15px; text-align: center; background: rgba(0, 255, 65, 0.05); }
        .stat-label { font-size: 0.75rem; text-transform: uppercase; opacity: 0.7; display: block; margin-bottom: 8px; }
        .stat-value { font-size: 1.3rem; font-weight: bold; }
        
        .registry-box { background: #001100; border: 1px solid var(--neon-green); padding: 20px; border-radius: 8px; margin-bottom: 40px; text-align: center; }
        .input-group { display: flex; gap: 10px; justify-content: center; margin-top: 15px; }
        input { background: #000; border: 1px solid var(--neon-green); color: var(--neon-green); padding: 10px; border-radius: 4px; width: 250px; }
        .btn { background: var(--neon-green); color: #000; border: none; padding: 10px 20px; font-weight: bold; cursor: pointer; border-radius: 4px; }
        .btn:hover { background: #fff; }
        .warning { color: #ff3e3e; font-size: 0.7rem; margin-top: 10px; text-transform: uppercase; letter-spacing: 1px; }

        .activity-section { margin-top: 40px; }
        .activity-header { font-size: 1.2rem; margin-bottom: 15px; border-left: 4px solid var(--neon-green); padding-left: 10px; text-transform: uppercase; }
        
        table { width: 100%; border-collapse: collapse; margin-top: 10px; font-size: 0.85rem; }
        th { text-align: left; padding: 12px; border-bottom: 2px solid var(--neon-green); opacity: 0.8; }
        td { padding: 12px; border-bottom: 1px solid rgba(0, 255, 65, 0.2); }
        tr:hover { background: rgba(0, 255, 65, 0.05); cursor: pointer; }
        
        .block-details { display: none; padding: 15px; background: #000; border: 1px dashed var(--neon-green); margin: 5px 0; font-size: 0.75rem; overflow-x: auto; }
        .block-details.active { display: block; }
        
        .footer { margin-top: 50px; text-align: center; font-size: 0.7rem; opacity: 0.5; }
        .live-indicator { display: inline-block; width: 10px; height: 10px; background: red; border-radius: 50%; margin-right: 10px; animation: blink 1s infinite; }
        @keyframes blink { 0% { opacity: 1; } 50% { opacity: 0; } 100% { opacity: 1; } }
    </style>
</head>
<body>
    <div class="container">
        <h1>👁️ SEER EXPLORER</h1>
        
        <div class="registry-box">
            <div style="font-weight: bold; font-size: 1rem; letter-spacing: 2px;">JOIN THE SWARM</div>
            <p style="font-size: 0.8rem; opacity: 0.8;">Register your bot to the global network signal.</p>
            <div class="input-group">
                <input type="text" id="bot-username" placeholder="@your_node_bot">
                <button class="btn" onclick="registerBot()">REGISTER NODE</button>
            </div>
            <div class="warning">⚠️ OPERATORS: MUTE THE MINER CHANNEL. HIGH-FREQUENCY DATA FEED.</div>
        </div>

        <div class="grid">
            <div class="stat-box"><span class="stat-label">Network Height</span><span class="stat-value" id="latest-block">0</span></div>
            <div class="stat-box"><span class="stat-label">Swarm Size</span><span class="stat-value" id="swarm-bots">0</span></div>
            <div class="stat-box"><span class="stat-label">Cloud Engines</span><span class="stat-value" id="cloud-engines">0</span></div>
            <div class="stat-box"><span class="stat-label">Local Engines</span><span class="stat-value" id="local-engines">0</span></div>
            <div class="stat-box"><span class="stat-label">Mined Supply</span><span class="stat-value" id="mined-supply">0</span></div>
            <div class="stat-box"><span class="stat-label">Percentage Mined</span><span class="stat-value" id="percent-mined">0.00%</span></div>
        </div>

        <div class="activity-section">
            <div class="activity-header">Recent Blocks</div>
            <table id="block-table">
                <thead>
                    <tr><th>Height</th><th>Hash</th><th>Miner</th><th>Time</th></tr>
                </thead>
                <tbody id="block-table-body"></tbody>
            </table>
        </div>

        <div class="footer">
            SEER NETWORK v1.0.0 | COORDINATOR: seer-coordinator.toon-satoshi.workers.dev
        </div>
    </div>

    <script>
        function toggleDetails(id) { document.getElementById(id).classList.toggle('active'); }

        async function registerBot() {
            const username = document.getElementById('bot-username').value;
            if(!username.startsWith('@')) { alert("Include @ in the username."); return; }
            
            const res = await fetch('https://seer-coordinator.toon-satoshi.workers.dev/register-bot', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ bot_username: username })
            });
            const data = await res.json();
            if(data.success) {
                alert("SUCCESS: Bot added to Swarm Registry!");
                location.reload();
            } else {
                alert("ERROR: " + data.error + "\\n" + data.fix);
            }
        }

        async function updateStats() {
            try {
                const res = await fetch('https://seer-coordinator.toon-satoshi.workers.dev/network-state?t=' + Date.now());
                const data = await res.json();
                
                document.getElementById('latest-block').textContent = data.latest_block;
                document.getElementById('swarm-bots').textContent = data.swarm_bots || 0;
                document.getElementById('cloud-engines').textContent = data.cloud_engines || 0;
                document.getElementById('local-engines').textContent = data.local_engines || 0;
                document.getElementById('percent-mined').textContent = (data.percent_mined || 0).toFixed(6) + '%';
                document.getElementById('mined-supply').textContent = (data.total_supply - 100000000).toLocaleString();

                const tbody = document.getElementById('block-table-body');
                if (data.recent_blocks && data.recent_blocks.length > 0) {
                    tbody.innerHTML = '';
                    data.recent_blocks.forEach((block, index) => {
                        const rowId = 'details-' + index;
                        const tr = document.createElement('tr');
                        tr.onclick = () => toggleDetails(rowId);
                        tr.innerHTML = '<td>#' + block.height + '</td><td style="opacity:0.6;font-size:0.7rem">' + block.hash.slice(0, 16) + '...</td><td>' + block.miner.slice(0, 8) + '...</td><td>' + new Date(block.timestamp * 1000).toLocaleTimeString() + '</td>';
                        
                        const detailsTr = document.createElement('tr');
                        detailsTr.innerHTML = '<td colspan="4" style="padding:0;border:none"><div class="block-details" id="' + rowId + '">' +
                                                '<strong>Full Hash:</strong> ' + block.hash + '<br>' +
                                                '<strong>Miner ID:</strong> ' + block.miner + '<br>' +
                                                '<strong>Nonce:</strong> ' + block.nonce + '<br>' +
                                                '<strong>Difficulty:</strong> ' + block.difficulty + ' bits' +
                                              '</div></td>';
                        tbody.appendChild(tr);
                        tbody.appendChild(detailsTr);
                    });
                }
            } catch (e) {}
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
