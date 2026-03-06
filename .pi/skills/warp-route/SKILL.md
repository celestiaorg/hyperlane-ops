---
name: warp-route
description: Use this skill when creating, updating, deploying, applying, verifying, or manually testing Hyperlane Warp Routes in this repository (including EVM and Celestia flows).
---

# Warp Route Skill

## When to use
Use this skill when the task involves any of the following in this repo:
- Adding or updating `deployments/warp_routes/<TOKEN>/*-deploy.yaml`
- Running `hyperlane warp deploy` or `hyperlane warp apply`
- Producing/updating `*-config.yaml` warp artifacts
- Updating `deployments/warp_routes/warpRouteConfigs.yaml`
- Verifying routers, standards, connections, and decimals
- Manual transfer debugging with `cast` (EVM) or `celestia-appd` (Celestia)

## Required user input intake
Before drafting configs or running any deploy/apply command, explicitly ask the user for missing route inputs.

Minimum required inputs:
- token symbol (e.g. `USDC`)
- chains in route (e.g. `sepolia`, `edentestnet`, `celestiatestnet`)
- canonical/collateral chain
- collateral token address or denom on canonical chain
- decimals per chain (or confirmation they are identical)
- route intent: new route vs update existing route

Also ask/confirm when unclear:
- token name
- owner addresses per chain
- mesh preference (full mesh vs constrained routing)
- whether docs/index updates are required

## Source of truth
Always use local docs and local registry files first:
- `docs/warp-routes.md`
- `docs/warp-route-multichain.md`
- `docs/manual-warp-transfer.md`
- `deployments/warp_routes/schema.json`
- `deployments/warp_routes/warpRouteConfigs.yaml`
- existing routes under `deployments/warp_routes/<TOKEN>/`

Do not rely on memory for chain names, domain IDs, token IDs, or router addresses; copy from repo files.

## Required environment
Before CLI operations, ensure:
- `HYP_KEY` is set for EVM-chain signing
- `HYP_KEY_COSMOSNATIVE` is set for Celestia/cosmosnative signing

Optional:
- `HYP_MNEMONIC` if direct `celestia-appd` key recovery is needed
- chain-specific keys like `HYP_CHAINS_<CHAIN>_SIGNER_KEY` from `.env`

## Deployer readiness gate (must run before deploy/apply)
Always verify deployer configuration before running `hyperlane warp deploy` or `hyperlane warp apply`.

Default behavior:
- use `HYP_KEY` for EVM chains
- use `HYP_KEY_COSMOSNATIVE` for Celestia/cosmosnative chains

If either required key is missing:
- stop and prompt the user to set it (or confirm an alternate signer flow)
- do not proceed with deploy/apply until signer configuration is explicit

## Workflow

### 1) Preflight
1. Confirm all required user inputs are collected (see intake section).
2. Confirm deployer readiness gate passes (`HYP_KEY` / `HYP_KEY_COSMOSNATIVE` as needed).
3. Confirm chain metadata exists under `chains/<chain>/metadata.yaml`.
4. Confirm target route naming convention and chain order in filename:
   - `deployments/warp_routes/<TOKEN>/<chainA>-<chainB>-...-deploy.yaml`
5. Confirm token role per chain (`collateral`, `synthetic`, or `native`) and decimals.

### 2) Author/update deploy config
1. Copy an existing route as template where possible.
2. Set `owner`, `type`, `token`/denom, `name`, `symbol`, `decimals`.
3. For cosmosnative chains, include fields required by local docs (e.g. `scale` when needed).

### 3) Deploy or apply
Deploy new route:
```bash
hyperlane warp deploy \
  --wd ./deployments/warp_routes/<TOKEN>/<route>-deploy.yaml \
  --wc ./deployments/warp_routes/<TOKEN>/<route>-config.yaml \
  --registry .
```

Apply update to existing route:
```bash
hyperlane warp apply \
  --symbol <TOKEN> \
  --config ./deployments/warp_routes/<TOKEN>/<route>-deploy.yaml \
  --registry .
```

### 4) Persist generated artifacts
1. Commit generated `*-config.yaml`.
2. Update `deployments/warp_routes/warpRouteConfigs.yaml` with the route block.
3. Verify the index update actually happened; if CLI did not write it, append the new block manually from generated config.

### 5) Verify
Read route:
```bash
hyperlane warp read --symbol <TOKEN> --registry .
```

Checks:
- token `standard` matches intended type per chain
- `collateralAddressOrDenom` exists on collateral route
- connections include expected peers (full mesh unless intentionally restricted)
- decimals and symbol/name match deploy config

If multiple routes share the same symbol, select the intended route ID when prompted.

### 6) Manual transfer/debug flows
Use `docs/manual-warp-transfer.md`:
- EVM-origin route calls via `cast` (`approve` where collateral applies, then `transferRemote`)
- Celestia-origin via `celestia-appd tx warp transfer`

Useful Celestia preflight queries:
```bash
celestia-appd query warp token <token-id> --node <rpc> -o json
celestia-appd query warp remote-routers <token-id> --node <rpc> -o json
celestia-appd query warp quote-transfer <token-id> <destination-domain> --node <rpc> -o json
```

## Guardrails
- Always pass `--registry .` for Hyperlane CLI commands in this repo.
- Never remove or overwrite unrelated warp routes when adding a new one.
- Never commit secrets/private keys.
- Keep route index and generated config in sync.
