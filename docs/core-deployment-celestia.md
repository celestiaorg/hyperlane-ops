# Celestia (Cosmosnative) Core Deployment

This guide covers deploying Hyperlane core on Celestia using the cosmosnative module and Hyperlane CLI. It also notes the canonical Mocha deployment tracked in this repo.

## Canonical Deployment (Mocha)
The canonical Celestia Mocha core deployment addresses are tracked in the local registry:
- `chains/celestiatestnet/addresses.yaml`

```yaml
interchainGasPaymaster: "0x726f757465725f706f73745f6469737061746368000000040000000000000003"
interchainSecurityModule: "0x726f757465725f69736d00000000000000000000000000040000000000000000"
mailbox: "0x68797065726c616e650000000000000000000000000000000000000000000000"
merkleTreeHook: "0x726f757465725f706f73745f6469737061746368000000030000000000000000"
```

## Prerequisites
- A funded cosmosnative deployer key exported as `HYP_KEY_COSMOSNATIVE`.
- A chain registry entry for the target Celestia chain.

```bash
export HYP_KEY_COSMOSNATIVE=0x...
```

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
hyperlane core deploy --registry . --chain celestiadev --config configs/celestia-core.yaml
```

3. Sync core configuration with the deployment:
```bash
hyperlane core read --registry . --chain celestiadev --config configs/celestia-core.yaml
```

## Updating a Core Deployment
You can update the core configuration (e.g., add a new domain to the `DomainRoutingIsm` or `IGP`). This requires the new chain to exist in the registry.

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

Apply updates and re-sync:
```bash
hyperlane core apply --registry . --chain celestiadev --config configs/celestia-core.yaml --key.cosmosnative=$HYP_KEY_COSMOSNATIVE
hyperlane core read --registry . --chain celestiadev --config configs/celestia-core.yaml
```

## Outputs
Deployed contract addresses are written to:
- `chains/<chain>/addresses.yaml`

The deployment config is updated by `hyperlane core read` to include deployed addresses.
