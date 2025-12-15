# hyperlane-ops

A repository that hosts Hyperlane deployment configurations and documentation for Celestia Mocha and Eden testnets.

## Prerequisites

- Install [Docker](https://www.docker.com/get-started/) for running the Relayer [docker-compose.yml](./docker-compose.yml).
- Install the official [Hyperlane CLI ](https://docs.hyperlane.xyz/docs/reference/developer-tools/cli).
- Install the [`celestia-appd`](https://github.com/celestiaorg/celestia-app) CLI binary.

## Repository Structure

- `registry/` holds the local Hyperlane registry with chain configs and canonical addresses used by CLI commands.
- `configs/` contains deployment YAMLs for core and warp routes.
- `solidity/` contains contracts used for native asset minting via Hyperlane.

## Testnet Deployments

Please refer to the [DEPLOYMENTS.md](./DEPLOYMENTS.md) file for detailed information about testnet Warp route deployments.

## EVM Core Deployment

The following outlines how to deploy a basic Hyperlane core contract stack on an EVM based blockchain network.
Note the `hyperlane` CLI binary expected an environment variable `HYP_KEY` containing the private key of the deployer account.

A prerequisite step required for deployments is to include a chain `metadata.yaml` file in the registry.
By default the `hyperlane` CLI uses the official Hyperlane registry but this can be overridden using a command line flag with a local registry.
Existing chain metadata for a wide range of networks is availabe in the official registry at https://github.com/hyperlane-xyz/hyperlane-registry.

A new custom chain registry entry can be initialised using the following command.

```bash
hyperlane registry init --registry ./registry
```

1. Run the following command to initialise a deployment config. Using the `--advanced` flag allows more fine-grained control over the deployment setup.

For basic testnet deployments we will use the following:
- DefaultISM: a `testIsm` which provides no security guarantees. 
- DefaultHook: a `protocolFee` hook which can be set to 0.
- RequiredHook: a `merkleTree` hook which inserts messages into an incremental merkle tree.

```bash
hyperlane core init --advanced --config ./configs/arbitrum-core.yaml --registry ./registry
```

2. Deploy the Hyperlane core contracts. This example uses `arbitrumsepolia` from the local chain registry.

```bash
hyperlane core deploy --chain arbitrumsepolia --config ./configs/arbitrum-core.yaml --registry ./registry
```

3. Read the core config on-chain artifacts and write them to the config file.

```bash
hyperlane core read --chain arbitrumsepolia --config ./configs/arbitrum-core.yaml --registry ./registry
```

## Celestia Mocha Core Deployment

The following configuration file specifies the canonical addresses for the Celestia Mocha Hyperlane core deployment.
A core deployment is composed of a `Mailbox`, configured with post-dispatch hooks for outbound message processing and an `InterchainSecurityModule (ISM)` for inbound message processing.

Using a single canonical deployment for the Hyperlane core stack on Celestia Mocha minimises operational overhead as maintaining multiple mailboxes and their associated post-dispatch hooks and ism configurations can become cumbersome.

See the [Hyperlane registry](./registry/) in this repository for the `addresses.yaml` configuration file.

```yaml
interchainGasPaymaster: "0x726f757465725f706f73745f6469737061746368000000040000000000000003"
interchainSecurityModule: "0x726f757465725f69736d00000000000000000000000000040000000000000000"
mailbox: "0x68797065726c616e650000000000000000000000000000000000000000000000"
merkleTreeHook: "0x726f757465725f706f73745f6469737061746368000000030000000000000000"
```

If you are running a local celestia chain setup for testing purposes, a new Hyperlane core stack can be deployed using the `celestia-appd` binary.

### Onboarding new connections

In order to create a new connnection (which is a prerequisite to Warp route deployment) the Hyperlane core deployment must be updated to support the remote chain's domain identifer. This is a two-step process:
- Register the domain identifier with a gas config in the `InterchainGasPaymaster (IGP)` destination gas configs .
- Register the domain identifier in the `InterchainSecurityModule (ISM)` domain routing config.

#### Adding a new domain to the `IGP` destination gas configs

Note that this is a requirement for _sending_ messages to a remote (counterparty) chain.

```bash
celestia-appd tx hyperlane hooks igp set-destination-gas-config [igp-id] [remote-domain] [token-exchange-rate] [gas-price] [gas-overhead] --from owner --fees 800utia
```

#### Adding a new domain ism to the `RoutingISM`

Note that this is a requirement for _receiving_ messages from a remote (counterparty) chain.

```bash
celestia-appd tx hyperlane ism set-routing-ism-domain [routing-ism-id] [domain] [ism-id] --from owner --fees 800utia
```

## Commands

TODO: This section will likely be removed but for now is used for dumping useful commands.
Currently a section to dump commands used to build up this repository.

Reading a core config from Eden.
```bash
hyperlane core read --chain edentestnet --config configs/eden-core.yaml --registry registry
```

Reading a core config from Celestia.
```bash
hyperlane core read --chain celestiatestnet --config configs/mocha-core.yaml --registry registry
```

Reading a warp config from Celestia.
```bash
hyperlane warp read --registry ./registry --config mocha-warp.yaml --chain celestiatestnet --address 0x726f757465725f61707000000000000000000000000000010000000000000006
```

## Warp Routes

### Creating an ERC20 collateral token using three chains

Run the following the commands and follow the interactive instructions on screen.

```
hyperlane warp init --registry ./registry
```

- Select three chains for example: `celestiatestnet,edentestnet,sepolia`.
- Enter the owner address for each deployment and choose either synthetic or collateral token. The suggested address will be derived from the key set in `HYP_KEY`, however when interacting with a `cosmosnative` module such as with Celestia mocha, we must explicitly override the account address suggested.
- Choose synthetic token types for both Celestia testnet and Eden testnet.
- Choose collateral token for the Sepolia Ethereum testnet and provide the token address of your ECR20.

A yaml deployment file will be created similar to the following. Note, if your ERC20 uses custom decimals, you must configure this manually in the generated yaml file.

```diff
celestiatestnet:
+  decimals: 6
+  scale: 100
  isNft: false
  owner: "celestia1lg0e9n4pt29lpq2k4ptue4ckw09dx0aujlpe4j"
  type: synthetic
edentestnet:
+  decimals: 8
  isNft: false
  owner: "0xc259e540167B7487A89b45343F4044d5951cf871"
  type: synthetic
sepolia:
+  decimals: 8
  owner: "0xc259e540167B7487A89b45343F4044d5951cf871"
  token: "0x0A3eC97CA4082e83FeB77Fa69F127F0eAABD016E"
  type: collateral
```

### Extending an existing Warp Route to add a new chain

Adding Eden testnet as a new synthetic on an existing warp route between Celestia Mocha and Sepolia.
Please note, the following set of steps will automatically enroll a remote router on Eden for connecting directly back to Sepolia.
In order to enforce that users go back to Sepolia via Celestia Mocha that remote router mapping must be unenrolled. This can be done manually using `cast` with the associated owner account.

1. Extend the deploy config file to add edentestnet.

```diff
sepolia:
  decimals: 18
  name: ETH
  owner: "0xc259e540167B7487A89b45343F4044d5951cf871"
  symbol: ETH
  type: native
celestiatestnet:
  decimals: 6
  scale: 1000000000000 # 10^12
  name: ETH
  owner: "celestia1lg0e9n4pt29lpq2k4ptue4ckw09dx0aujlpe4j"
  symbol: ETH
  token: ETH
  type: synthetic
+ edentestnet:
+   decimals: 18
+   name: ETH
+   owner: "0xc259e540167B7487A89b45343F4044d5951cf871"
+   symbol: ETH
+   token: ETH
+   type: synthetic
```

2. Run the `hyperlane warp apply` command providing the deploy conifg and the output config file paths as well as the local registry.

```bash
hyperlane warp apply --config ./registry/deployments/warp_routes/ETH/sepolia-mocha-deploy.yaml --wc ./registry/deployments/warp_routes/ETH/sepolia-mocha-config.yaml --registry ./registry
```

> [!IMPORTANT]  
> Oberserve the logs outputted by the command above. In the event that the `MsgEnrollRemoteRouter` fails on celestia it must be done manually.

3. Manually enroll the remote router on celestiatestnet.

```bash
celestia-appd tx warp enroll-remote-router 0x726f757465725f6170700000000000000000000000000002000000000000001d 2147483647 0x000000000000000000000000f8e7A4608AE1e77743FD83549b36E605213760b6 0 --from hyp-owner --fees 800utia
```
