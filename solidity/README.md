# Hyperlane Solidity Tooling

This directory contains the Foundry setup for `HypNativeMinter` and its Hyperlane and OpenZeppelin dependencies.

## Prerequisites
- [Foundry](https://getfoundry.sh/) (`forge`, `cast`). Install and keep it up to date with `foundryup`.

## Install Dependencies
Run the following from the `solidity/` directory to fetch vendored libraries:

```bash
forge install
```

This pulls the repos listed in `foundry.toml` into `lib/`.

## Compile Contracts
```bash
forge build
```

Build artifacts are written to `out/`, which is ignored from Git.

## Run Tests
```bash
forge test
```

Add tests under `solidity/test/` as needed.

## Format Contracts
```bash
forge fmt
```

## Update Dependencies
To update the libraries in `lib/`, use:

```bash
forge update
```

## Clean Build Outputs
```bash
forge clean
```

This removes `out/` and `cache/`.
