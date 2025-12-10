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

## Celestia Mocha Core Deployment

The following configuration file specifies the canonical addresses for the Celestia Mocha Hyperlane core deployment.
A core deployment is composed of a `Mailbox`, configured with post-dispatch hooks for outbound message processing and an `InterchainSecurityModule (ISM)` for inbound message processing.

See the [Hyperlane registry](./registry/) in this repository for the `addresses.yaml` configuration file.

```yaml
interchainGasPaymaster: "0x726f757465725f706f73745f6469737061746368000000040000000000000003"
interchainSecurityModule: "0x726f757465725f69736d00000000000000000000000000040000000000000000"
mailbox: "0x68797065726c616e650000000000000000000000000000000000000000000000"
merkleTreeHook: "0x726f757465725f706f73745f6469737061746368000000030000000000000000"
```

## EVM Core Deployment

The following outlines how to deploy a basic Hyperlane core contract stack on an EVM based blockchain network.
Note the `hyperlane` CLI binary expected an environment variable `HYP_KEY` containing the private key of the deployer account.

A prerequisite step required for deployments is to include a chain `metadata.yaml` file in the registry.
By default the `hyperlane` CLI uses the official Hyperlane registry but this can be overridden using a command line flag with a local registry.
Existing chain metadata for a wide range of networks is availabe in the official registry at https://github.com/hyperlane-xyz/hyperlane-registry.

1. Run the following command to initialise a deployment config. Using the `--advanced` flag allows more fine-grained control over the deployment setup.

For basic testnet deployments we will use the following:
- DefaultISM: a `testIsm` which provides no security guarantees. 
- DefaultHook: a `protocolFee` hook which can be set to 0.
- RequiredHook: a `merkleTree` hook which inserts messages into an incremental merkle tree.

```bash
hyperlane core init --registry ./registry --advanced
```

2. Deploy the Hyperlane core contracts. This example uses `arbitrumsepolia` from the local chain registry.

```bash
hyperlane core deploy --registry ./registry --chain arbitrumsepolia
```

3. Read the core config on-chain artifacts and write them to the config file.

```bash
hyperlane core read --chain arbitrumsepolia --config ./configs/arbitrum-core.yaml --registry ./registry
```

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
