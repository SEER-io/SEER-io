#!/bin/bash

# SEER Node Setup Script

set -e

echo "--- SEER Network Node Setup ---"

# 1. Check Dependencies
for cmd in curl unzip cargo; do
    if ! command -v $cmd &> /dev/null; then
        echo "Error: $cmd is not installed. Please install it and try again."
        exit 1
    fi
done

# 2. Prompt for BOT_TOKEN if not provided as env var
if [ -z "$BOT_TOKEN" ]; then
    read -p "Enter your Telegram Bot Token: " BOT_TOKEN
fi

if [ -z "$BOT_TOKEN" ]; then
    echo "Error: BOT_TOKEN is required."
    exit 1
fi

# 3. Derive Node ID (First 12 hex chars of SHA-256 of bot token)
NODE_ID=$(echo -n "$BOT_TOKEN" | sha256sum | awk '{print $1}' | cut -c1-12)
echo "Derived Node ID: $NODE_ID"

# 4. Extract Node Binaries (Placeholder action)
if [ -f "seer_node.zip" ]; then
    echo "Extracting seer_node.zip..."
    unzip -o seer_node.zip
else
    echo "Note: seer_node.zip not found. Assuming development environment."
fi

# 5. Write Configuration
mkdir -p config
cat <<EOF > config/genesis.toml
[telegram]
bot_token = "$BOT_TOKEN"
node_id = "$NODE_ID"

[network]
genesis_supply = 100000000
block_time = 10
EOF

echo "Configuration written to config/genesis.toml"

# 6. Register Node with Coordinator
COORDINATOR_URL="https://seer-coordinator.workers.dev/register" # Placeholder URL

echo "Registering node with coordinator..."
RESPONSE=$(curl -s -X POST "$COORDINATOR_URL" \
    -H "Content-Type: application/json" \
    -d "{\"bot_token\": \"$BOT_TOKEN\", \"node_id\": \"$NODE_ID\"}")

echo "Registration Response: $RESPONSE"

# 7. Start the Node
echo "Setup complete. Starting SEER node..."
# cargo run --release # Uncomment to run actual binary
echo "Run 'cargo run --release' to start your node."
