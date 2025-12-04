# hyperlane-ops

A repository that hosts Hyperlane deployment configurations and documentation for Celestia Mocha and Eden testnets.

## Prerequisites

TODO: Fill out prereqs for installing

- Docker (link)
- Hyperlane CLI (link)

## Celestia Mocha Mailbox Config

See the local hyperlane registry in this repository for the `addresses.yaml` configuration file.

```yaml
interchainGasPaymaster: "0x726f757465725f706f73745f6469737061746368000000040000000000000003"
interchainSecurityModule: "0x726f757465725f69736d00000000000000000000000000040000000000000000"
mailbox: "0x68797065726c616e650000000000000000000000000000000000000000000000"
merkleTreeHook: "0x726f757465725f706f73745f6469737061746368000000030000000000000000"
```

The above configuration file specifies the canonical addresses for the Celestia Mocha Hyperlane core deployment.
This is composed of a `Mailbox` configured with post-disptach hooks for outbound message processing and an `InterchainSecurityModule (ISM)` for inbound message processing.

### Onboarding new connections

In order to create a new connnection (which is a prerequisite to Warp route deployment) the Hyperlane core deployment must be updated to support the remote chain's domain identifer. This is a two-step process:
- Register the domain identifier with a gas config in the `InterchainGasPaymaster (IGP)` destination gas configs .
- Register the domain identifier in the `InterchainSecurityModule (ISM)` domain routing config.

#### Adding a new domain to the `IGP` destination gas configs

Note that this is a requirement for _sending_ messages to a remote (counterparty) chain.

TODO: Update this...
```bash
celestia-appd tx hyperlane hooks igp set-destination-gas-config [igp-id] [remote-domain] [token-exchange-rate] [gas-price] [gas-overhead] [flags]
```

#### Adding a new domain ism to the `RoutingISM`

Note that this is a requirement for _receiving_ messages from a remote (counterparty) chain.

```bash
celestia-appd tx hyperlane ism set-routing-ism-domain 0x726f757465725f69736d00000000000000000000000000010000000000000000 [domain] [ism-id] --from owner-key --fees 800utia
```

## Commands

Currently a section to dump commands used to build up this repository.

Reading a core config from Eden.
```bash
hyperlane core read --chain edentestnet --config configs/eden-core.yaml --registry registry
```

Reading a core config from Celestia.
```bash
hyperlane core read --chain celestiamocha --config configs/mocha-core.yaml --registry registry
```

Reading a warp config from Celestia.
```bash
hyperlane warp read --registry ./registry --config mocha-warp.yaml --chain celestiamocha --address 0x726f757465725f61707000000000000000000000000000010000000000000006
```

