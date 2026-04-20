#!/bin/bash
set -euo pipefail

export HYP_KEY=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
export HYP_KEY_COSMOSNATIVE=0x6e30efb1d3ebd30d1ba08c8d5fc9b190e08394009dc1dd787a69e60c33288a8c

echo "Using Hyperlane registry:"
hyperlane registry list --registry ./registry

echo "Deploying Hyperlane core on rethlocal..."
hyperlane core deploy --chain rethlocal --config ./configs/rethlocal-core.yaml --registry ./registry --yes
hyperlane core read --chain rethlocal --config ./configs/rethlocal-core.yaml --registry ./registry

echo "Deploying Hyperlane core on celestiadev..."
hyperlane core deploy --chain celestiadev --config ./configs/celestia-core.yaml --registry ./registry --yes
hyperlane core read --chain celestiadev --config ./configs/celestia-core.yaml --registry ./registry

echo "Deploying TIA warp route..."
hyperlane warp deploy --warp-route-id TIA --registry ./registry --yes
