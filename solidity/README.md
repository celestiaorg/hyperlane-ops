# Hyperlane Solidity Tooling

This directory contains the Foundry setup for `HypNativeMinter` and its Hyperlane and OpenZeppelin dependencies.

## Prerequisites

- [Foundry](https://getfoundry.sh/) (`forge`, `cast`). Install and keep it up to date with `foundryup`.

## Usage

### Install Dependencies

Run the following from the `solidity/` directory to fetch vendored libraries:

```bash
forge soldeer install
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

A forge deployment script is included to deploy the `HypNativeMinter` contract.
The script assumes that the `$PRIVATE_KEY` used for deployment is also the `mintAdmin` of the Evolve precompile.

Configure the deployment parameters via environment variables:

- `MAILBOX_ADDRESS`
- `NATIVE_MINTER_PRECOMPILE` (optional, defaults to `0x000000000000000000000000000000000000F100`)
- `HYP_NATIVE_SCALE` (optional, defaults to `1e12`)

Then run the deployment script (replace `RPC_URL` and `ADMIN_KEY` as needed):

```bash
forge script script/DeployHypNativeMinter.s.sol \
  --rpc-url $RPC_URL \
  --private-key $ADMIN_KEY \
  --broadcast
```

Omit `--broadcast` to run a dry-run against a fork or local node.

Once the `HypNativeMinter` is deployed, add it to the allow list of the native minter precompile.
Where `ADMIN_KEY` is the `mintAdmin` as per EVM `genesis.json`.

```bash
cast send --rpc-url $RPC_URL --private-key $ADMIN_KEY \
  0x000000000000000000000000000000000000F100 \
  "addToAllowList(address)" 0x...deadbeef
```

Validate the `HypNativeMinter` contract is an authorized minter.

```bash
cast call --rpc-url $RPC_URL \
  0x000000000000000000000000000000000000F100 \
  "allowlist(address)(bool)" 0x...deadbeef
```

Ensure the remote router is set for both the `HypNativeMinter` (evm) and collateral token in Celestia.
Where `ADMIN_KEY` is the owner of the Hyperlane contract deployment.

The `enrollRemoteRouter` method accepts both the destination domain identifier and token identifier/contract address as parameters.
For example, the `utia` collateral token identifier on Celestia using the domain id `69420`.

```
cast send 0x...deadbeef \
  "enrollRemoteRouter(uint32,bytes32)" \
  69420 0x726f757465725f61707000000000000000000000000000010000000000000000 \
  --private-key $ADMIN_KEY \
  --rpc-url $RPC_URL
```

Likewise, the collateral token on the remote counterpart should set its remote router to the `HypNativeMinter` contract address accordingly.
For example, set the `utia` collateral token's remote router to the `HypNativeMinter` contract address using domain id 1234. Ensure the 20 byte contract address is left-padded to satisfy the 32 byte requirement.

```
celestia-appd tx warp enroll-remote-router 0x726f757465725f61707000000000000000000000000000010000000000000000 1234 0x00000000000000000000000083b466f5856dC4F531Bb5Af45045De06889D63CB 0 --from $OWNER --fees 800utia
```
