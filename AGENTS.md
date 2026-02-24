# AGENTS

## Purpose
This repo is a local Hyperlane registry plus ops configs for Celestia Mocha and Eden testnets. It tracks chain metadata and core addresses, warp route deployments, relayer and faucet configs, and Solidity tooling for HypNativeMinter.

## Authorization
The user must provide access and approval to the agent to run CLI tooling on their behalf.

## Source of Truth
- Use the local docs in `docs/` and the local registry files in this repo as the primary references for all workflows.
- Prefer repository state over memory: copy chain names, domain IDs, token IDs, and router addresses from local files before running commands.
- Core docs to consult first:
  - `docs/warp-routes.md`
  - `docs/warp-route-multichain.md`
  - `docs/manual-warp-transfer.md`
  - `docs/celestia-core-deploy.md`
  - `docs/evm-core-deploy.md`
  - `docs/relayer.md`

## Repository Structure
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
3. Commit the generated `deployments/warp_routes/<TOKEN>/*-config.yaml`.
4. Update `deployments/warp_routes/warpRouteConfigs.yaml` with the new/updated route entry.
5. Verify the index update was actually written; if not, append the route block manually from the generated config.
6. Add/update token logos in deployments/warp_routes/<TOKEN>/logo.svg as needed.

### Manual Transfers
- For manual warp transfers (EVM + Celestia), use `docs/manual-warp-transfer.md`.
- EVM-origin transfers use `cast` against the router `transferRemote(...)` flow.
- Celestia-origin transfers use `celestia-appd tx warp transfer`.

### Refresh On-Chain Configs
Use `hyperlane core read` and `hyperlane warp read` to refresh files in configs/.
- If multiple routes share a symbol (for example multiple `USDC` routes), `hyperlane warp read --symbol <TOKEN>` will prompt for route selection.

### Celestia-Specific Ops
For new domain onboarding, update IGP destination gas configs and Routing ISM entries with `celestia-appd` commands documented in:
- `docs/celestia-core-deploy.md`
- `docs/manual-warp-transfer.md`

### Relayer
Update relayer/config.json and keep relayChains in sync with the chain entries. Start with `docker compose up -d` from the repo root.

### Faucets
Run `docker compose up -d` in faucets/. Config lives in faucets/*/faucet-config.yaml.

### Solidity
Run Foundry commands inside solidity/ (`forge build`, `forge test`, `forge script`).

## Secrets and Safety
The repository contains a `.env.example` file at the repo root. Copy it to `.env` and set sensitive values there.
The `.env` file is git ignored and must not be committed.
- Do not commit private keys (HYP_KEY, HYP_CHAINS_*_SIGNER_KEY, faucet ethWalletKey).
- Use environment variables or a secret manager for runtime values.

## Deployment and operations automation
### Environment Variables
The user must configure private keys if they wish to allow the agent to operate CLIs on their behalf.

Set the keys needed for CLI operations before running commands:

- Required for Hyperlane CLI on EVM chains: `HYP_KEY`
- Required for Hyperlane CLI on Celestia (`cosmosnative`) chains: `HYP_KEY_COSMOSNATIVE`
- Optional for direct `celestia-appd` usage: `HYP_MNEMONIC` (recover a local keyring entry such as `owner`)
- Optional chain-specific signer keys in `.env`: `HYP_CHAINS_<CHAIN>_SIGNER_KEY`

Example setup:
```bash
export HYP_KEY=0x...
export HYP_KEY_COSMOSNATIVE=0x...
export HYP_MNEMONIC="word1 word2 ... word24"

# Recover or import the Celestia key into the local keyring
echo "$HYP_MNEMONIC" | celestia-appd keys add owner --recover
# or
celestia-appd keys import owner <key-file>
```

### Debugging
Useful resources for debugging errors when operating the CLI tools.
- [Hyperlane Cosmos Runbook](https://hyperlanexyz.notion.site/Runbook-Hyperlane-Cosmos-SDK-2b06d35200d681f2a3c0e481a45b9275)
- [Hyperlane Docs](https://docs.hyperlane.xyz/)

## Validation
- `forge test` for solidity changes.
- `hyperlane core read` / `hyperlane warp read` after updating deployments.
- `docker compose ps` and `docker logs` for relayer and faucet health checks.
