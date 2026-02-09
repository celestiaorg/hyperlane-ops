# Celestia Mocha Core Deployment

The following configuration file specifies the canonical addresses for the Celestia Mocha Hyperlane core deployment.
A core deployment is composed of a `Mailbox`, configured with post-dispatch hooks for outbound message processing and an `InterchainSecurityModule (ISM)` for inbound message processing.

Using a single canonical deployment for the Hyperlane core stack on Celestia Mocha minimizes operational overhead, as maintaining multiple mailboxes and their associated post-dispatch hooks and ISM configurations can become cumbersome.

See the Celestia registry entry in this repository at `chains/celestiatestnet/addresses.yaml`.

```yaml
interchainGasPaymaster: "0x726f757465725f706f73745f6469737061746368000000040000000000000003"
interchainSecurityModule: "0x726f757465725f69736d00000000000000000000000000040000000000000000"
mailbox: "0x68797065726c616e650000000000000000000000000000000000000000000000"
merkleTreeHook: "0x726f757465725f706f73745f6469737061746368000000030000000000000000"
```

If you are running a local Celestia chain for testing, a new Hyperlane core stack can be deployed using the `celestia-appd` binary.
