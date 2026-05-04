---
name: core-deploy
description: Use this skill when deploying, applying, updating, verifying, or debugging Hyperlane core deployments in this repository, including EVM and cosmosnative/Celestia flows.
---

# Core Deploy Skill

## When to use
Use this skill when the task involves any of the following in this repo:
- Adding or updating Hyperlane core deployment config files under `configs/<chain>-core.yaml`
- Running `hyperlane core init`, `hyperlane core deploy`, `hyperlane core apply`, or `hyperlane core read`
- Updating `chains/<chain>/addresses.yaml` or aggregated `chains/addresses.yaml` with core addresses
- Configuring Mailbox default ISM, default hook, required hook, IGP, or domain routing ISM
- Onboarding a new remote domain to a Celestia/cosmosnative core deployment
- Verifying core addresses, hook/ISM configuration, relayer readiness, or gas/ISM routing

## Required user input intake
Before drafting configs or running deploy/apply commands, explicitly ask the user for missing core deployment inputs.

Minimum required inputs:
- target chain name from this repo, e.g. `edentestnet`, `sepolia`, `celestiatestnet`
- protocol path: `ethereum`/EVM or `cosmosnative`
- deploy intent: new deployment vs update existing deployment
- owner address for the chain/protocol address format
- target security posture: testnet/dev vs mainnet/production

Also ask/confirm when unclear:
- desired default ISM (`testIsm`, `domainRoutingIsm`, `merkleRootMultisig`, etc.)
- desired default hook (`protocolFee` or `interchainGasPaymaster`)
- desired required hook (`merkleTreeHook` or a composed/aggregation hook)
- remote domains/chains to register in `domainRoutingIsm` and IGP configs
- IGP `beneficiary`, `oracleKey`, `oracleConfig`, `overhead`, gas price, and token exchange rate values
- validator addresses and threshold for multisig ISM setup
- whether ownership is a multisig and whether the CLI can sign the needed changes
- whether `relayer/config.json` and docs/indexes should be updated after deployment

## Source of truth
Treat this repository as the canonical Hyperlane registry. Use local registry files for chain names, domain IDs, protocol types, RPC URLs, owners, and deployed addresses.

When behavior, commands, config shape, or operational details are unspecified or unclear, refer to the local docs in `docs/` before proceeding.

## Required environment
Before CLI operations, ensure:
- `HYP_KEY` is set for EVM-chain signing
- `HYP_KEY_COSMOSNATIVE` is set for Celestia/cosmosnative signing

Optional:
- `HYP_MNEMONIC` if direct `celestia-appd` key recovery is needed
- chain-specific keys like `HYP_CHAINS_<CHAIN>_SIGNER_KEY` from `.env`

Never commit secrets/private keys. The user must authorize CLI tooling before the agent runs it on their behalf.

## Deployer readiness gate
Always verify deployer configuration before running `hyperlane core deploy` or `hyperlane core apply`.

Default behavior:
- use `HYP_KEY` for EVM chains
- use `HYP_KEY_COSMOSNATIVE` for cosmosnative chains

If the required key is missing:
- stop and prompt the user to set it or confirm an alternate signer/multisig flow
- do not proceed with deploy/apply until signer configuration is explicit

For mainnet or multisig-owned deployments:
- do not assume CLI `core apply` can mutate ownership-gated config
- generate or prepare the exact multisig transactions when needed
- prefer dry-run/read-only validation before proposing writes

## Workflow

### 1) Preflight
1. Confirm required user inputs are collected.
2. Read `chains/<chain>/metadata.yaml` and confirm `name`, `domainId`, `chainId`, `protocol`, RPC URLs, native token, and gas settings.
3. Read existing `chains/<chain>/addresses.yaml` and `configs/<chain>-core.yaml` if present.
4. Confirm the chain exists in aggregated `chains/metadata.yaml`; if adding a new chain, keep per-chain and aggregate files aligned with `chains/schema.json`.
5. Confirm deployer readiness for the protocol.
6. Confirm intended config file path: `configs/<chain>-core.yaml`.
7. Use `--registry .` on every Hyperlane CLI command in this repo.

### 2) EVM core deploy path
Use this path when `chains/<chain>/metadata.yaml` has `protocol: ethereum`.

Recommended testnet/dev defaults:
- `defaultIsm: testIsm` (no production security guarantees)
- `defaultHook: protocolFee` (often fee `0`)
- `requiredHook: merkleTreeHook`

Recommended mainnet/production shape:
- `defaultIsm: domainRoutingIsm` with per-domain ISMs, commonly `merkleRootMultiSig`
- `defaultHook: interchainGasPaymaster` with per-domain gas config
- `requiredHook: merkleTreeHook` or a deliberate required aggregation hook

Initialize config:
```bash
hyperlane core init --advanced --config configs/<chain>-core.yaml --registry .
```

Deploy:
```bash
hyperlane core deploy --chain <chain> --config configs/<chain>-core.yaml --registry .
```

Read back on-chain state:
```bash
hyperlane core read --chain <chain> --config configs/<chain>-core.yaml --registry .
```

Expected outputs:
- deployed addresses in `chains/<chain>/addresses.yaml`
- refreshed core artifact in `configs/<chain>-core.yaml`

After deploy/read, mirror address changes into `chains/addresses.yaml` if the CLI did not update the aggregate file.

### 3) Cosmosnative core deploy path
Use this path when `chains/<chain>/metadata.yaml` has `protocol: cosmosnative` (for example Celestia or Noble).

For cosmosnative chains, prefer existing local registry metadata or official registry metadata over interactive `hyperlane registry init` prompts.

Author `configs/<chain>-core.yaml` explicitly. The typical Celestia shape is:
- `owner`: bech32 owner address
- `defaultHook.type: interchainGasPaymaster`
- `defaultHook.beneficiary`: bech32 recipient for gas payments
- `defaultHook.oracleKey`: bech32 gas oracle updater key
- `defaultHook.oracleConfig.<remote-chain>` with `gasPrice`, `tokenExchangeRate`, and `tokenDecimals` when required by the local config shape
- `defaultHook.overhead.<remote-chain>`
- `defaultIsm.type: domainRoutingIsm`
- `defaultIsm.domains.<remote-chain>` with the desired ISM, often `testIsm` in testnets
- `requiredHook.type: merkleTreeHook`

Deploy:
```bash
hyperlane core deploy --chain <chain> --config configs/<chain>-core.yaml --registry .
```

Read back on-chain state:
```bash
hyperlane core read --chain <chain> --config configs/<chain>-core.yaml --registry .
```

Expected outputs:
- deployed module IDs/addresses in `chains/<chain>/addresses.yaml`
- refreshed core artifact in `configs/<chain>-core.yaml`

After deploy/read, mirror address changes into `chains/addresses.yaml` if the CLI did not update the aggregate file.

### 4) Updating existing core deployment
Use `core apply` for deliberate config changes after reading current on-chain state first.

Read current state:
```bash
hyperlane core read --chain <chain> --config configs/<chain>-core.yaml --registry .
```

Edit `configs/<chain>-core.yaml` with the intended changes only.

Apply:
```bash
hyperlane core apply --chain <chain> --config configs/<chain>-core.yaml --registry .
```

Read again:
```bash
hyperlane core read --chain <chain> --config configs/<chain>-core.yaml --registry .
```

For IGP-only recurring updates, avoid broad automated `core apply` unless the signer is intended to own broad core config. Prefer direct, narrowly scoped IGP/oracle setter transactions when automation should only update gas pricing.

### 5) Celestia remote-domain onboarding
Before a Warp Route can reliably send between Celestia and a new remote chain, Celestia core needs both:
- a routing ISM entry for the remote domain (inbound verification)
- an IGP destination gas config for the remote domain (outbound gas quoting/payment)

Use `docs/new-chain-onboarding.md` for the Celestia transaction flow:

Create a Merkle root multisig ISM when using multisig security:
```bash
celestia-appd tx hyperlane ism create-merkle-root-multisig <validators> <threshold> <flags>
```

Register the ISM for the remote domain:
```bash
celestia-appd tx hyperlane ism set-routing-ism-domain <routing-ism-id> <domain> <ism-id> <flags>
```

Set destination gas config:
```bash
celestia-appd tx hyperlane hooks igp set-destination-gas-config <igp-id> <remote-domain> <token-exchange-rate> <gas-price> <gas-overhead> <flags>
```

For multisig-owned Celestia deployments, use `--generate-only` where appropriate and have multisig participants sign externally.

### 6) Relayer follow-up
After adding or changing a core chain, check whether `relayer/config.json` needs the chain in `relayChains` and matching chain settings.

Useful checks:
```bash
docker compose ps
docker logs hyperlane-relayer -f
```

## Verification checklist
After deploy/apply/read:
- `configs/<chain>-core.yaml` contains addresses for deployed hooks/ISMs where applicable
- `chains/<chain>/addresses.yaml` contains expected core addresses such as `mailbox`, `merkleTreeHook`, `interchainGasPaymaster`, `interchainSecurityModule`, factories, or `validatorAnnounce`
- `chains/addresses.yaml` mirrors per-chain address changes
- `hyperlane core read --chain <chain> --config configs/<chain>-core.yaml --registry .` succeeds
- default ISM and hook choices match the intended security posture
- `domainRoutingIsm` has all required remote domains
- IGP has all required remote destination gas configs
- relayer config is updated if the chain should be relayed

For EVM-specific verification, use `cast call` against the chain RPC when needed to confirm Mailbox owner/hooks or contract getters.

For cosmosnative-specific verification, use `celestia-appd query hyperlane ...` commands from the docs to confirm ISM, hook, IGP, and routing module state when CLI read output is insufficient.

## Guardrails
- Always pass `--registry .` for Hyperlane CLI commands in this repo.
- Do not overwrite unrelated chain addresses, configs, or warp route artifacts.
- Do not commit `.env` or private keys.
- Treat `testIsm` as testnet/dev-only unless the user explicitly accepts its security properties.
- For `domainRoutingIsm` and `interchainGasPaymaster`, missing remote-domain entries can break inbound verification or outbound gas quoting.
- For required safety controls, prefer `requiredHook`; `defaultHook` may be overrideable by callers depending on dispatch path.
- Read before apply, apply only intentional changes, then read again.
