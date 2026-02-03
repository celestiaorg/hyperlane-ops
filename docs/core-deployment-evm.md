# EVM Core Deployment

The following outlines how to deploy a basic Hyperlane core contract stack on an EVM-based blockchain network.
The `hyperlane` CLI expects an environment variable `HYP_KEY` containing the private key of the deployer account.

## Registry Prerequisites
A chain `metadata.yaml` file must exist in the registry before deployment.
The CLI defaults to the official registry, but it can be overridden with the `--registry .` flag to use this local registry.

Initialize a new local registry entry:
```bash
hyperlane registry init --registry .
```

## Core Deployment Workflow
1. Initialize a deployment config. Use `--advanced` for fine-grained control.

For basic testnet deployments, the defaults are:
- DefaultISM: `testIsm` (no security guarantees)
- DefaultHook: `protocolFee` (can be set to 0)
- RequiredHook: `merkleTree` (inserts messages into an incremental merkle tree)

```bash
hyperlane core init --advanced --config ./configs/arbitrum-core.yaml --registry .
```

2. Deploy the core contracts (example uses `arbitrumsepolia` from local registry):
```bash
hyperlane core deploy --chain arbitrumsepolia --config ./configs/arbitrum-core.yaml --registry .
```

3. Read core config on-chain artifacts back into the config file:
```bash
hyperlane core read --chain arbitrumsepolia --config ./configs/arbitrum-core.yaml --registry .
```
