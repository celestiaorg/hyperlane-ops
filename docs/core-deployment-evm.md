# EVM Core Deployment

This guide covers deploying Hyperlane core contracts on an EVM-based chain using the Hyperlane CLI. It assumes you are using the local registry in this repo.

## Prerequisites
- A funded EVM account private key exported as `HYP_KEY`.
- A chain registry entry in `chains/<chain>/metadata.yaml` (or initialize via CLI).

```bash
export HYP_KEY=0x...
```

## Registry Prerequisites
The CLI defaults to the official registry, but you can override it with `--registry .` to use this local registry.

Initialize a local registry entry (auto-detects a local EVM RPC at `http://localhost:8545`):
```bash
hyperlane registry init --registry .
```

## Core Deployment Workflow
1. Initialize a deployment config. Use `--advanced` for fine-grained control.

For basic testnet deployments, the recommended defaults are:

- DefaultISM: `testIsm` (no security guarantees)
- DefaultHook: `protocolFee` (can be set to 0)
- RequiredHook: `merkleTreeHook` (inserts messages into an incremental merkle tree)

```bash
hyperlane core init --advanced --config configs/evolve-core.yaml --registry .
```

2. Deploy the core contracts (example selects a chain via prompt):
```bash
hyperlane core deploy --registry . --config configs/evolve-core.yaml
```

3. Read core config on-chain artifacts back into the config file:
```bash
hyperlane core read --registry . --chain evolve --config configs/evolve-core.yaml
```

## Outputs
Deployed contract addresses are written to:

- `chains/<chain>/addresses.yaml`

The deployment config is updated by `hyperlane core read` to include deployed addresses.
