# AGENTS

## Purpose
This repo is a local Hyperlane registry plus ops configs for Celestia Mocha and Eden testnets. It tracks chain metadata and core addresses, warp route deployments, relayer and faucet configs, and Solidity tooling for HypNativeMinter.

## Start Here
- README.md for repo overview, Hyperlane CLI usage, and relayer ops.
- DEPLOYMENTS.md for testnet warp routes and token addresses.
- faucets/README.md for Eden faucet stack details.
- solidity/README.md for Foundry workflows and HypNativeMinter deployment.

## Repository Map
- chains/
  - chains/<chain>/metadata.yaml and chains/<chain>/addresses.yaml are the per-chain registry entries.
  - chains/metadata.yaml and chains/addresses.yaml are aggregated views; keep them in sync with per-chain files.
  - chains/schema.json defines the chain metadata schema.
- deployments/warp_routes/
  - <TOKEN>/*-deploy.yaml are input configs for `hyperlane warp deploy` or `hyperlane warp apply`.
  - <TOKEN>/*-config.yaml are output configs (token connections and addresses).
  - deployments/warp_routes/warpRouteConfigs.yaml is the index used by the registry.
  - deployments/warp_routes/schema.json defines the warp route config schema.
- configs/
  - On-chain read artifacts for core and warp routes (`hyperlane core read` and `hyperlane warp read`).
- relayer/
  - relayer/config.json is the Hyperlane agent configuration.
  - docker-compose.yml at repo root runs the relayer container.
- faucets/
  - faucets/docker-compose.yml and faucet-config.yaml files for the PoWFaucet stack.
- solidity/
  - Foundry project for HypNativeMinter, with tests and deploy scripts.

## Common Workflows
### Use the Local Registry
Pass `--registry .` to Hyperlane CLI commands.

### Add or Update a Chain
1. Edit chains/<name>/metadata.yaml and chains/<name>/addresses.yaml.
2. Mirror changes in chains/metadata.yaml and chains/addresses.yaml.
3. Keep values aligned with chains/schema.json and existing naming conventions.

### Add or Update a Warp Route
1. Update deployments/warp_routes/<TOKEN>/*-deploy.yaml.
2. Run `hyperlane warp deploy` or `hyperlane warp apply` to produce a new config.
3. Update deployments/warp_routes/<TOKEN>/*-config.yaml and deployments/warp_routes/warpRouteConfigs.yaml.
4. Add/update token logos in deployments/warp_routes/<TOKEN>/logo.svg as needed.

### Refresh On-Chain Configs
Use `hyperlane core read` and `hyperlane warp read` to refresh files in configs/.

### Celestia-Specific Ops
For new domain onboarding, update IGP destination gas configs and Routing ISM entries with `celestia-appd` commands documented in README.md.

### Relayer
Update relayer/config.json and keep relayChains in sync with the chain entries. Start with `docker compose up -d` from the repo root.

### Faucets
Run `docker compose up -d` in faucets/. Config lives in faucets/*/faucet-config.yaml.

### Solidity
Run Foundry commands inside solidity/ (`forge build`, `forge test`, `forge script`).

## Secrets and Safety
- Do not commit private keys (HYP_KEY, HYP_CHAINS_*_SIGNER_KEY, faucet ethWalletKey).
- Use environment variables or a secret manager for runtime values.

## Validation
- `forge test` for solidity changes.
- `hyperlane core read` / `hyperlane warp read` after updating deployments.
- `docker compose ps` and `docker logs` for relayer and faucet health checks.
