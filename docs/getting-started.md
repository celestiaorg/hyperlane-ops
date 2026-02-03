# Getting Started

## Prerequisites
- Install Docker for running the relayer `docker-compose.yml` at the repo root.
- Install the Hyperlane CLI.
- Install the `celestia-appd` CLI binary.

Reference links:
- https://www.docker.com/get-started/
- https://docs.hyperlane.xyz/docs/reference/developer-tools/cli
- https://github.com/celestiaorg/celestia-app

## Use the Local Registry
Pass `--registry .` to Hyperlane CLI commands to use the local registry contained in this repository.

Example:
```bash
hyperlane core read --chain edentestnet --config configs/eden-core.yaml --registry .
```

## Environment Variables
To operate CLIs on your behalf, configure private keys in the environment:
- `HYP_KEY` for Hyperlane CLI EVM deployments
- `HYP_MNEMONIC` for Celestia key recovery/import

Example:
```bash
export HYP_KEY=0x...
export HYP_MNEMONIC="word1 word2 ... word24"

# Recover or import the Celestia key into the local keyring
echo $HYP_MNEMONIC | celestia-appd keys add owner --recover
# or
celestia-appd keys import owner <key-file>
```

## Secrets and Safety
- Do not commit private keys (`HYP_KEY`, `HYP_CHAINS_*_SIGNER_KEY`, faucet `ethWalletKey`).
- Use environment variables or a secret manager for runtime values.
- `.env` is git ignored; use `.env.example` as the template.

## Validation Shortlist
- `forge test` for solidity changes
- `hyperlane core read` / `hyperlane warp read` after updating deployments
- `docker compose ps` and `docker compose logs` for relayer and faucet health checks
