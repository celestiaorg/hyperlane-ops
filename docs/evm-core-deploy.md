# EVM Core Deployment

This guide covers deploying Hyperlane core contracts on an EVM-based chain using the Hyperlane CLI. It assumes you are using the local registry in this repo.

## Prerequisites
- Hyperlane CLI installed.
- A funded EVM account private key exported as `HYP_KEY`.
- A chain registry entry in `chains/<chain>/metadata.yaml`.

```bash
export HYP_KEY=0x...
```

## Registry Prerequisites
The Hyperlane CLI defaults to the [official registry](https://github.com/hyperlane-xyz/hyperlane-registry), but can be overridden it with the `--registry` flag to use the [celestia registry](https://github.com/celestiaorg/hyperlane-ops).

Initialize a local registry  using the CLI (auto-detects a local EVM RPC at `http://localhost:8545`):
```bash
hyperlane registry init --registry .
```

## Core Deployment Workflow

For **testnet** deployments, the recommended defaults are:

- DefaultISM: `testIsm` (no security guarantees)
- DefaultHook: `protocolFee` (can be set to 0)
- RequiredHook: `merkleTreeHook` (inserts messages into an incremental merkle tree)

For **mainnet** deployemnts, the recommended defaults are:

- DefaultISM: `domainRoutingIsm` (configure ism for remote chains per-domain)
    - with `merkleRootMultiSig` (see [MultisigISM](./multisig-ism.md) for details)
- DefaultHook: `interchainGasPaymaster` (configure gas fee overhead for remote chains per-domain)
- RequiredHook: `merkleTreeHook` (inserts messages into an incremental merkle tree)

!!! note
    The `domainRoutingIsm` and `interchainGasPaymaster` hook must register ISMs and gas configs for remote chain domain identifiers. This is a requirement for processing both inbound and outbound messages.

1. Initialize a deployment config. Use `--advanced` for fine-grained control. This will produce a `<chain>-core.yaml` file in the `configs` directory.
```bash
hyperlane core init --advanced --config configs/<chain>-core.yaml --registry .
```

2. Deploy the core contracts (omitting `--chain` allows selection via prompt).
```bash
hyperlane core deploy --chain <chain>  --config configs/<chain>-core.yaml --registry .
```

3. Read core config on-chain artifacts back into the config file.
```bash
hyperlane core read --registry . --chain <chain> --config configs/<chain>-core.yaml
```

## Outputs
Deployed contract addresses are written to:

- `chains/<chain>/addresses.yaml`

The deployment config is updated by `hyperlane core read` to include deployed addresses.
