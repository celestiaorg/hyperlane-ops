# Hyperlane Solidity Tooling

This directory contains the Foundry setup for `HypNativeMinter` and its Hyperlane and OpenZeppelin dependencies.

## Prerequisites
- [Foundry](https://getfoundry.sh/) (`forge`, `cast`). Install and keep it up to date with `foundryup`.

## Usage

### Install Dependencies
Run the following from the `solidity/` directory to fetch vendored libraries:

```bash
forge install
```

This pulls the repos listed in `foundry.toml` into `lib/`.

### Compile Contracts
```bash
forge build
```

Build artifacts are written to `out/`, which is ignored from Git.

### Run Tests
```bash
forge test
```

Add tests under `solidity/test/` as needed.

### Format Contracts
```bash
forge fmt
```

### Update Dependencies
To update the libraries in `lib/`, use:

```bash
forge update
```

### Clean Build Outputs
```bash
forge clean
```

This removes `out/` and `cache/`.

## Deploy HypNativeMinter
Configure the deployment parameters via environment variables:

- `NATIVE_MINTER_PRECOMPILE` (optional, defaults to `0x000000000000000000000000000000000000F100`)
- `HYP_NATIVE_SCALE`
- `MAILBOX_ADDRESS`

Then run the deployment script (replace `RPC_URL` and `PRIVATE_KEY` as needed):

```bash
forge script script/DeployHypNativeMinter.s.sol \
  --rpc-url $RPC_URL \
  --private-key $PRIVATE_KEY \
  --broadcast
```

Omit `--broadcast` to run a dry-run against a fork or local node.

TODO: consider extending the deployment script to also call `enrollRemoteRouter(domain, addr)` with the TIA domain and collateral token address.
