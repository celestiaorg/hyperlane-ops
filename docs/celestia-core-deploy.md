# Celestia Core Deployment

This guide covers deploying Hyperlane core on Celestia using the `cosmosnative` module and Hyperlane CLI.
Please note that existing deployments already exist for mainnet and testnets. However, this guide may be useful development environments and debugging. For onboarding new chains to existing deployments please see [Onboarding new chains to Celestia](./new-chain-onboarding.md).

## Prerequisites
- Hyperlane CLI installed.
- A funded Celestia deployer key exported as `HYP_KEY_COSMOSNATIVE`.
- A chain registry entry for the target Celestia chain.

```bash
export HYP_KEY_COSMOSNATIVE=0x...
```

!!! note
    For cosmosnative chains (e.g., Celestia), it is recommended to copy chain metadata from the official Hyperlane registry rather than relying on the interactive `hyperlane registry init` prompt.

## Core Deployment Workflow
1. Create a deployment config file (example below). This defines the default hooks and ISM.

Create `configs/celestia-core.yaml`:
```yaml
defaultHook:
  beneficiary: celestia1y3kf30y9zprqzr2g2gjjkw3wls0a35pfs3a58q
  oracleConfig:
    evolve:
      gasPrice: "1000000000"
      tokenDecimals: 18
      tokenExchangeRate: "1"
  oracleKey: celestia1y3kf30y9zprqzr2g2gjjkw3wls0a35pfs3a58q
  overhead:
    evolve: 300000
  owner: celestia1y3kf30y9zprqzr2g2gjjkw3wls0a35pfs3a58q
  type: interchainGasPaymaster
defaultIsm:
  domains:
    evolve:
      type: testIsm
  owner: celestia1y3kf30y9zprqzr2g2gjjkw3wls0a35pfs3a58q
  type: domainRoutingIsm
owner: celestia1y3kf30y9zprqzr2g2gjjkw3wls0a35pfs3a58q
requiredHook:
  type: merkleTreeHook
```

2. Deploy the core infrastructure:
```bash
hyperlane core deploy --chain celestiadev --config configs/celestia-core.yaml --registry .
```

3. Read core config on-chain artifacts back into the config file.
```bash
hyperlane core read --chain celestiadev --config configs/celestia-core.yaml --registry .
```

## Updating a Core Deployment
You can update the core configuration (e.g., add a new domain to the `DomainRoutingIsm` or `IGP`). This requires the new chain to exist in the registry.

!!! warning
    Since the mainnet deployments are owned by a multisig account, the following cannot be done using the CLI tooling and requires a transaction to be signed by the multisig participants. However this is appropriate to use for testnets and developer environments.

Example diff adding a `mockchain` entry:
```diff
defaultHook:
  address: "0x726f757465725f706f73745f6469737061746368000000040000000000000000"
  beneficiary: celestia1y3kf30y9zprqzr2g2gjjkw3wls0a35pfs3a58q
  oracleConfig:
    evolve:
      gasPrice: "1000000000"
      tokenDecimals: 18
      tokenExchangeRate: "1"
+    mockchain:
+      gasPrice: "100000000"
+      tokenDecimals: 6
+      tokenExchangeRate: "1"
  oracleKey: celestia1y3kf30y9zprqzr2g2gjjkw3wls0a35pfs3a58q
  overhead:
    evolve: 300000
+    mockchain: 300000
  owner: celestia1y3kf30y9zprqzr2g2gjjkw3wls0a35pfs3a58q
  type: interchainGasPaymaster
defaultIsm:
  address: "0x726f757465725f69736d00000000000000000000000000010000000000000001"
  domains:
    evolve:
      address: "0x726f757465725f69736d00000000000000000000000000000000000000000000"
      type: testIsm
+    mockchain:
+      type: testIsm
  owner: celestia1y3kf30y9zprqzr2g2gjjkw3wls0a35pfs3a58q
  type: domainRoutingIsm
owner: celestia1y3kf30y9zprqzr2g2gjjkw3wls0a35pfs3a58q
requiredHook:
  address: "0x726f757465725f706f73745f6469737061746368000000030000000000000001"
  type: merkleTreeHook
```

1. Apply the updates to the deployment spec.
```bash
hyperlane core apply --registry . --chain celestiadev --config configs/celestia-core.yaml
```

2. Read core config on-chain artifacts back into the config file.
```bash
hyperlane core read --registry . --chain celestiadev --config configs/celestia-core.yaml
```

## Outputs
Deployed contract addresses are written to:

- `chains/<chain>/addresses.yaml`

The deployment config is updated by `hyperlane core read` to include deployed addresses.
