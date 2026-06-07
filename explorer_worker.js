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
        
        .activity-section { margin-top: 40px; }
        .activity-header { font-size: 1.2rem; margin-bottom: 15px; border-left: 4px solid var(--neon-green); padding-left: 10px; text-transform: uppercase; }
        
        table { width: 100%; border-collapse: collapse; margin-top: 10px; font-size: 0.85rem; }
        th { text-align: left; padding: 12px; border-bottom: 2px solid var(--neon-green); opacity: 0.8; }
        td { padding: 12px; border-bottom: 1px solid rgba(0, 255, 65, 0.2); }
        tr:hover { background: rgba(0, 255, 65, 0.05); cursor: pointer; }
        
        .block-details { display: none; padding: 15px; background: #000; border: 1px dashed var(--neon-green); margin: 5px 0; font-size: 0.75rem; overflow-x: auto; }
        .block-details.active { display: block; }
        .hash-link { color: var(--neon-green); opacity: 0.6; font-size: 0.7rem; }
        
        .footer { margin-top: 50px; text-align: center; font-size: 0.7rem; opacity: 0.5; }
        .live-indicator { display: inline-block; width: 10px; height: 10px; background: red; border-radius: 50%; margin-right: 10px; animation: blink 1s infinite; }
        @keyframes blink { 0% { opacity: 1; } 50% { opacity: 0; } 100% { opacity: 1; } }
    </style>
</head>
<body>
    <div class="container">
        <h1>👁️ SEER EXPLORER</h1>
        <p style="text-align: center;"><span class="live-indicator"></span>ORACLE-ENHANCED LIVE FEED</p>
        
        <div class="grid">
            <div class="stat-box"><span class="stat-label">Latest Height</span><span class="stat-value" id="latest-block">0</span></div>
            <div class="stat-box"><span class="stat-label">Cloud Engines</span><span class="stat-value" id="cloud-engines">0</span></div>
            <div class="stat-box"><span class="stat-label">Local Engines</span><span class="stat-value" id="local-engines">0</span></div>
            <div class="stat-box"><span class="stat-label">Percentage Mined</span><span class="stat-value" id="percent-mined">0.00%</span></div>
            <div class="stat-box"><span class="stat-label">Total Supply</span><span class="stat-value" id="total-supply">0</span></div>
            <div class="stat-box"><span class="stat-label">Market Cap Proxy</span><span class="stat-value" id="mcap" style="color: #ff0000;">$0.00M</span></div>
            <div class="stat-box"><span class="stat-label">Network Velocity</span><span class="stat-value" id="velocity">0.000</span></div>
            <div class="stat-box"><span class="stat-label">Gini Index</span><span class="stat-value" id="gini">0.00000</span></div>
        </div>

        <div class="activity-section">
            <div class="activity-header">Recent Blocks</div>
            <table>
                <thead>
                    <tr>
                        <th>Height</th>
                        <th>Hash</th>
                        <th>Miner</th>
                        <th>Time</th>
                    </tr>
                </thead>
                <tbody id="block-table-body">
                </tbody>
            </table>
        </div>

        <div class="footer">
            SEER NETWORK v1.0.0 | COORDINATOR: seer-coordinator.toon-satoshi.workers.dev
        </div>
    </div>

    <script>
        function toggleDetails(id) {
            const el = document.getElementById(id);
            el.classList.toggle('active');
        }

        async function updateStats() {
            try {
                const res = await fetch('https://seer-coordinator.toon-satoshi.workers.dev/network-state?t=' + Date.now());
                const data = await res.json();
                
                document.getElementById('latest-block').textContent = data.latest_block;
                document.getElementById('cloud-engines').textContent = data.cloud_engines || 0;
                document.getElementById('local-engines').textContent = data.local_engines || 0;
                document.getElementById('percent-mined').textContent = (data.percent_mined || 0).toFixed(6) + '%';
                document.getElementById('total-supply').textContent = (data.total_supply / 1000000).toFixed(4) + 'M';
                document.getElementById('velocity').textContent = (data.velocity || 0.002).toFixed(3);
                document.getElementById('gini').textContent = (data.gini || 0.35).toFixed(5);

                const tbody = document.getElementById('block-table-body');
                if (data.recent_blocks && data.recent_blocks.length > 0) {
                    tbody.innerHTML = '';
                    data.recent_blocks.forEach((block, index) => {
                        const rowId = 'details-' + index;
                        const time = new Date(block.timestamp * 1000).toLocaleTimeString();
                        
                        const tr = document.createElement('tr');
                        tr.onclick = () => toggleDetails(rowId);
                        tr.innerHTML = '<td>#' + block.height + '</td>' +
                                       '<td class="hash-link">' + block.hash.slice(0, 16) + '...</td>' +
                                       '<td>' + block.miner.slice(0, 8) + '...</td>' +
                                       '<td>' + time + '</td>';
                        
                        const detailsTr = document.createElement('tr');
                        detailsTr.innerHTML = '<td colspan="4" style="padding: 0; border: none;">' +
                                                '<div class="block-details" id="' + rowId + '">' +
                                                    '<strong>Full Hash:</strong> ' + block.hash + '<br>' +
                                                    '<strong>Miner ID:</strong> ' + block.miner + '<br>' +
                                                    '<strong>Nonce:</strong> ' + block.nonce + '<br>' +
                                                    '<strong>Difficulty:</strong> ' + block.difficulty + ' bits<br>' +
                                                    '<strong>Timestamp:</strong> ' + block.timestamp + ' (' + new Date(block.timestamp*1000).toISOString() + ')' +
                                                '</div>' +
                                              '</td>';
                        
                        tbody.appendChild(tr);
                        tbody.appendChild(detailsTr);
                    });
                }
            } catch (e) {
                console.error('Update failed:', e);
            }
        }
        updateStats();
        setInterval(updateStats, 5000);
    </script>
</body>
</html>\`;
    
  return new Response(html, {
    headers: { "Content-Type": "text/html" }
  })
}
