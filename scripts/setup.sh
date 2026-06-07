#!/bin/bash
# SEER Network - Master Onboarding Script
# Version: 2.0 (June 2026)

set -e

echo "------------------------------------------------"
echo "👁️ SEER NETWORK - OPERATOR ONBOARDING"
echo "------------------------------------------------"

# 1. Choose Setup Path
echo "Choose your node setup type:"
echo "1) Local Node (Run on this machine/Termux - No Cloudflare required)"
echo "2) Cloudflare Node (Automated GitHub Actions deployment)"
read -p "Selection [1-2]: " SETUP_TYPE

if [ "$SETUP_TYPE" == "1" ]; then
    echo -e "\n--- Local Node Configuration ---"
    read -p "Telegram Bot Token: " TG_BOT_TOKEN
    read -p "Custom Node Name (optional): " NODE_NAME
    NODE_NAME=${NODE_NAME:-"SEER Local Node"}

    cat <<EOF > local_node_config.json
{
  "bot_token": "$TG_BOT_TOKEN",
  "node_name": "$NODE_NAME"
}
EOF
    echo -e "\n✅ Configuration saved to local_node_config.json"
    echo "To start your node, run:"
    echo "node scripts/run-local.js"
    echo -e "\n💡 TIP: Once running, type /apply to your bot to join the Global Miner Channel!"

elif [ "$SETUP_TYPE" == "2" ]; then
    echo -e "\n--- Cloudflare CI/CD Configuration ---"
    read -p "Cloudflare Account ID: " CF_ACCOUNT_ID
    read -p "Cloudflare API Token: " CF_API_TOKEN
    read -p "Telegram Bot Token: " TG_BOT_TOKEN
    read -p "GitHub PAT (with 'repo' scope): " GH_PAT
    read -p "GitHub Repo (e.g., owner/repo): " GH_REPO

    echo -e "\n--- Automating GitHub Secrets ---"
    
    # Python helper for secret encryption
    cat <<EOF > .update_secrets.py
import requests, sys, base64
from nacl import encoding, public

def encrypt(public_key, secret_value):
    public_key = public.PublicKey(public_key.encode("utf-8"), encoding.Base64Encoder())
    sealed_box = public.SealedBox(public_key)
    encrypted = sealed_box.encrypt(secret_value.encode("utf-8"))
    return base64.b64encode(encrypted).decode("utf-8")

def update_secret(repo, token, name, val, k_id, pub_key):
    url = f"https://api.github.com/repos/{repo}/actions/secrets/{name}"
    headers = {"Authorization": f"Bearer {token}", "Accept": "application/vnd.github+json"}
    data = {"encrypted_value": encrypt(pub_key, val), "key_id": k_id}
    r = requests.put(url, headers=headers, json=data)
    return r.status_code

if __name__ == "__main__":
    repo, token, cf_id, cf_token, bot_token = sys.argv[1:6]
    pk_res = requests.get(f"https://api.github.com/repos/{repo}/actions/secrets/public-key", 
                          headers={"Authorization": f"Bearer {token}"}).json()
    if "key_id" not in pk_res:
        print("❌ Error: Could not fetch GitHub public key. Check repo and PAT.")
        sys.exit(1)
    k_id, pub_key = pk_res["key_id"], pk_res["key"]
    
    secrets = {"CLOUDFLARE_ACCOUNT_ID": cf_id, "CLOUDFLARE_API_TOKEN": cf_token, "BOT_TOKEN": bot_token, "CF_PROJECT_NAME": "seer-network"}
    for n, v in secrets.items():
        status = update_secret(repo, token, n, v, k_id, pub_key)
        print(f"  - Updated {n}: {status}")
EOF

    python3 .update_secrets.py "$GH_REPO" "$GH_PAT" "$CF_ACCOUNT_ID" "$CF_API_TOKEN" "$TG_BOT_TOKEN"
    rm .update_secrets.py

    echo -e "\n✅ Cloudflare setup complete."
    echo "1. Go to Cloudflare Dashboard > Workers & Pages."
    echo "2. ENABLE the 'workers.dev' subdomain for your nodes."
    echo "3. Push to GitHub to trigger the first deploy."
    echo -e "\n💡 TIP: Once live, type /apply to your bot to join the Global Miner Channel!"

else
    echo "Invalid selection."
    exit 1
fi
