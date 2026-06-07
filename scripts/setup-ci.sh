#!/bin/bash
# SEER Network - CI/CD & Cloudflare Automation Script
# This script automates the setting up of GitHub Secrets and Wrangler configuration.

set -e

echo "------------------------------------------------"
echo "👁️ SEER NETWORK - OPERATOR AUTOMATION"
echo "------------------------------------------------"

# Check for dependencies
if ! command -v jq &> /dev/null; then echo "Error: jq is required."; exit 1; fi

# 1. Gather Credentials
echo "Please enter your Cloudflare Credentials:"
read -p "Cloudflare Account ID: " CF_ACCOUNT_ID
read -p "Cloudflare API Token: " CF_API_TOKEN
read -p "Telegram Bot Token: " TG_BOT_TOKEN
read -p "GitHub PAT (with 'repo' scope): " GH_PAT
read -p "GitHub Repo (e.g., owner/repo): " GH_REPO

echo -e "\n--- Validating Cloudflare Token ---"
CF_VALID=$(curl -s -X GET "https://api.cloudflare.com/client/v4/user/tokens/verify" \
     -H "Authorization: Bearer $CF_API_TOKEN" \
     -H "Content-Type:application/json" | jq -r '.success')

if [ "$CF_VALID" != "true" ]; then
    echo "❌ Cloudflare Token is invalid. Please check your permissions."
    exit 1
else
    echo "✅ Cloudflare Token Verified."
fi

# 2. Automate GitHub Secrets
echo -e "\n--- Updating GitHub Secrets ---"
# We'll use a simple python helper to avoid installing heavy local dependencies
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
    print(f"  - Updated {name}: {r.status_code}")

if __name__ == "__main__":
    repo, token, cf_id, cf_token, bot_token = sys.argv[1:6]
    pk_res = requests.get(f"https://api.github.com/repos/{repo}/actions/secrets/public-key", 
                          headers={"Authorization": f"Bearer {token}"}).json()
    k_id, pub_key = pk_res["key_id"], pk_res["key"]
    
    secrets = {"CLOUDFLARE_ACCOUNT_ID": cf_id, "CLOUDFLARE_API_TOKEN": cf_token, "BOT_TOKEN": bot_token}
    for n, v in secrets.items():
        update_secret(repo, token, n, v, k_id, pub_key)
EOF

python3 .update_secrets.py "$GH_REPO" "$GH_PAT" "$CF_ACCOUNT_ID" "$CF_API_TOKEN" "$TG_BOT_TOKEN"
rm .update_secrets.py

# 3. Manual Steps Notice
echo -e "\n------------------------------------------------"
echo "✅ CI/CD AUTOMATION COMPLETE"
echo "------------------------------------------------"
echo "Next steps for the Operator:"
echo "1. Go to Cloudflare Dashboard > Workers & Pages."
echo "2. Find 'seer-node-001' and 'seer-coordinator'."
echo "3. Go to Settings > Triggers and ENABLE the 'workers.dev' subdomain."
echo "4. Push your changes to GitHub to trigger the first deploy."
echo "------------------------------------------------"
