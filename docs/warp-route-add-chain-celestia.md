# Add a Chain to an Existing Celestia Warp Route

Use this guide when you already have a Warp Route that includes `celestiatestnet` and you want to add one more chain to that same route.

This workflow updates an existing route (not a brand-new route) using `hyperlane warp apply`.

## Prerequisites

- Hyperlane CLI installed.
- Deployer keys configured:
  - `HYP_KEY` for EVM chains.
  - `HYP_KEY_COSMOSNATIVE` for Celestia (`cosmosnative`) interactions.
- The new chain already exists in this repo registry:
  - `chains/<new-chain>/metadata.yaml`
  - `chains/<new-chain>/addresses.yaml`
- Hyperlane core is already deployed on both Celestia and the new chain.

List available chains:

```bash
hyperlane registry list --registry .
```

## Step 1: Confirm Celestia Core Is Ready for the New Domain

Before updating the Warp Route, ensure Celestia core has:

- a routing ISM entry for the new remote domain, and
- an IGP destination gas config for the new remote domain.

If these are missing, message dispatch/delivery can fail even if `warp apply` succeeds.

Follow [Onboarding new chains to Celestia](./new-chain-onboarding.md) first if needed.

## Step 2: Update the Route Deploy Spec

Pick the existing route deploy file you want to extend:

```bash
ls deployments/warp_routes/<TOKEN>/*-deploy.yaml
```

Edit that file (or copy it to a new filename that includes the new chain name) and add a new top-level block for the chain.

Example: extending `USDC` from `celestiatestnet-edentestnet-sepolia` to also include `nobletestnet`:

```diff
 celestiatestnet:
   decimals: 6
   name: USDC
   owner: "celestia1..."
   symbol: USDC
   token: USDC
   type: synthetic
 edentestnet:
   decimals: 6
   name: USDC
   owner: "0x..."
   symbol: USDC
   token: USDC
   type: synthetic
+nobletestnet:
+  decimals: 6
+  name: USDC
+  owner: "noble1..."
+  symbol: USDC
+  token: USDC
+  type: synthetic
```

Keep the existing chain blocks (for example `sepolia`) unchanged, and only add the new chain block.

Role/field guidance:

- EVM synthetic: usually `type: synthetic` and `token: <SYMBOL>`.
- EVM collateral: `type: collateral` and `token: <ERC20_ADDRESS>`.
- Cosmosnative collateral (for native denoms): `type: collateral` and denom-style token where required (for example `utia` on Celestia).
- Cosmosnative synthetic: `type: synthetic`; add `scale` when decimals differ from the collateral representation.

## Step 3: Apply the Updated Route

Run:

```bash
hyperlane warp apply \
  --symbol <TOKEN> \
  --config ./deployments/warp_routes/<TOKEN>/<route>-deploy.yaml \
  --registry .
```

Notes:

- If multiple routes share the same symbol, the CLI may prompt you to select the target route.
- Keep `--registry .` so the command uses this local registry.

## Step 4: Persist Generated Artifacts

After apply:

1. Ensure the updated `*-config.yaml` is present under `deployments/warp_routes/<TOKEN>/`.
2. Update `deployments/warp_routes/warpRouteConfigs.yaml`.
3. Verify the route index was actually written; if not, append the route block manually from the generated config.

Quick check:

```bash
rg "<TOKEN>/<route>" deployments/warp_routes/warpRouteConfigs.yaml
```

## Step 5: Verify From Celestia Side

Read the route:

```bash
hyperlane warp read --symbol <TOKEN> --registry .
```

Then confirm Celestia router state includes the new remote chain.

Set:

```bash
CELESTIA_TOKEN_ID=<addressOrDenom for celestiatestnet in the route config>
NEW_DOMAIN=<domainId from chains/<new-chain>/metadata.yaml>
CELESTIA_RPC=https://rpc-1.testnet.celestia.nodes.guru
```

Query:

```bash
celestia-appd query warp remote-routers $CELESTIA_TOKEN_ID --node $CELESTIA_RPC -o json
celestia-appd query warp quote-transfer $CELESTIA_TOKEN_ID $NEW_DOMAIN --node $CELESTIA_RPC -o json
```

Expected result:

- `remote-routers` includes `NEW_DOMAIN`.
- `quote-transfer` returns a valid quote for `NEW_DOMAIN`.

## Step 6: Run a Smoke Transfer (Recommended)

Run one small transfer between Celestia and the new chain to validate end-to-end behavior.

Use [Manual Warp Transfer](./manual-warp-transfer.md) patterns:

- EVM origin: `cast ... transferRemote(...)`
- Celestia origin: `celestia-appd tx warp transfer ...`

## Troubleshooting

- `No router enrolled for domain`: route was not applied to the intended route instance, or enrollment failed.
- `quote-transfer` fails for the new domain: Celestia IGP destination gas config is likely missing.
- Delivery stalls after dispatch: relayer not running or destination gas config too low.
- Amount/decimals look wrong on destination: missing or incorrect `scale` for cosmosnative synthetic entries.
- Celestia Warp Route owner is a multisig:
  - Transfer ownership to a regular EOA to use Hyperlane CLI `warp apply` and update generated configuration files through the normal CLI flow.
  - Or generate and execute multisig transactions with `celestia-appd`, then update route configuration files manually in this repo.
