#!/bin/bash
set -euo pipefail

export HYP_KEY=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
export HYP_KEY_COSMOSNATIVE=0x6e30efb1d3ebd30d1ba08c8d5fc9b190e08394009dc1dd787a69e60c33288a8c
BOOTSTRAP_MARKER=/home/celestia/.celestia-app/.hyperlane_devnet_bootstrapped

if [[ -f "$BOOTSTRAP_MARKER" ]]; then
    echo "Hyperlane devnet bootstrap already completed, skipping redeploy."
    exit 0
fi

if [[ -z "${HYP_VALIDATOR_CHECKPOINT_KEY:-}" ]]; then
    echo "HYP_VALIDATOR_CHECKPOINT_KEY must be set for devnet multisig validation" >&2
    exit 1
fi

VALIDATOR_ADDRESS="$(cast wallet address --private-key "$HYP_VALIDATOR_CHECKPOINT_KEY")"

rewrite_validator_set() {
    local config_path="$1"
    local tmp_path
    tmp_path="$(mktemp)"

    awk -v validator="$VALIDATOR_ADDRESS" '
        /validators:/ {
            print
            getline
            indent = match($0, /[^ ]/) - 1
            if (indent < 0) indent = 8
            printf "%*s- \"%s\"\n", indent, "", validator
            next
        }
        { print }
    ' "$config_path" >"$tmp_path"

    mv "$tmp_path" "$config_path"
}

yaml_value() {
    local key="$1"
    local yaml_path="$2"

    awk -F': ' -v key="$key" '$1 == key {gsub(/"/, "", $2); print $2}' "$yaml_path"
}

echo "Using Hyperlane registry:"
hyperlane registry list --registry ./registry

echo "Preparing multisig validator set for $VALIDATOR_ADDRESS..."
rewrite_validator_set ./configs/anvil-core.yaml
rewrite_validator_set ./configs/celestia-core.yaml

echo "Deploying Hyperlane core on anvil..."
hyperlane core deploy --chain anvil --config ./configs/anvil-core.yaml --registry ./registry --yes

echo "Deploying Hyperlane core on celestiadev..."
hyperlane core deploy --chain celestiadev --config ./configs/celestia-core.yaml --registry ./registry --yes

echo "Applying multisig core configuration..."
hyperlane core apply --chain anvil --config ./configs/anvil-core.yaml --registry ./registry --yes
hyperlane core apply --chain celestiadev --config ./configs/celestia-core.yaml --registry ./registry --yes

echo "Reading deployed core configuration..."
hyperlane core read --chain anvil --config ./configs/anvil-core.yaml --registry ./registry
hyperlane core read --chain celestiadev --config ./configs/celestia-core.yaml --registry ./registry

echo "Syncing agent config with freshly deployed anvil addresses..."
export ANVIL_MAILBOX="$(yaml_value mailbox ./registry/chains/anvil/addresses.yaml)"
export ANVIL_MERKLE_TREE_HOOK="$(yaml_value merkleTreeHook ./registry/chains/anvil/addresses.yaml)"
export ANVIL_VALIDATOR_ANNOUNCE="$(yaml_value validatorAnnounce ./registry/chains/anvil/addresses.yaml)"
export ANVIL_INTERCHAIN_GAS_PAYMASTER="$(yaml_value interchainGasPaymaster ./registry/chains/anvil/addresses.yaml)"
export ANVIL_PROXY_ADMIN="$(yaml_value proxyAdmin ./registry/chains/anvil/addresses.yaml)"
export ANVIL_TEST_RECIPIENT="$(yaml_value testRecipient ./registry/chains/anvil/addresses.yaml)"
export ANVIL_INTERCHAIN_ACCOUNT_ROUTER="$(yaml_value interchainAccountRouter ./registry/chains/anvil/addresses.yaml)"

node <<'NODE'
const fs = require('fs');

const path = './agent-config.json';
const config = JSON.parse(fs.readFileSync(path, 'utf8'));
const anvil = config.chains.anvil;

anvil.mailbox = process.env.ANVIL_MAILBOX;
anvil.merkleTreeHook = process.env.ANVIL_MERKLE_TREE_HOOK;
anvil.validatorAnnounce = process.env.ANVIL_VALIDATOR_ANNOUNCE;
anvil.interchainGasPaymaster = process.env.ANVIL_INTERCHAIN_GAS_PAYMASTER;
anvil.proxyAdmin = process.env.ANVIL_PROXY_ADMIN;
anvil.testRecipient = process.env.ANVIL_TEST_RECIPIENT;
anvil.interchainAccountRouter = process.env.ANVIL_INTERCHAIN_ACCOUNT_ROUTER;

fs.writeFileSync(path, `${JSON.stringify(config, null, 2)}\n`);
NODE

if hyperlane warp read --symbol TIA --registry ./registry >/dev/null 2>&1; then
    echo "Applying TIA warp route..."
    hyperlane warp apply --symbol TIA --config ./registry/deployments/warp_routes/TIA/celestiadev-anvil-deploy.yaml --registry ./registry --yes
else
    echo "Deploying TIA warp route..."
    hyperlane warp deploy --warp-route-id TIA --registry ./registry --yes
fi

touch "$BOOTSTRAP_MARKER"
