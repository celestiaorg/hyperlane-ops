# Celestia Connection Onboarding

To create a new connection (a prerequisite for warp route deployment), update the Hyperlane core deployment to support the remote chain's domain identifier.
This is a three-step process:
1. Create a new ism for processing messages from a remote chain.
2. Register the domain identifier in the Interchain Security Module (ISM) domain routing config.
3. Register the domain identifier with a gas config in the Interchain Gas Paymaster (IGP) destination gas configs.

## Step 1: Create Merkle Root Multisig ISM
Create the multisig ISM on Celestia with the validator addresses and threshold.

```bash
celestia-appd tx hyperlane ism create-merkle-root-multisig [validators] [threshold] [flags]
```

Example:

```bash
celestia-appd tx hyperlane ism create-merkle-root-multisig 0xdead..beef,0xabec..fe32,0xabee...00dd 2 --signer owner --fees 800utia
```

Notes:
- The `validators` arg is a comma-separated list of `0x...` addresses.
- The multisig ISM validator list must match the ECDSA checkpoint signing keys used by the validator agents.

## Step 2: Set Routing ISM Domain
Add the new `MerkleRootMultisigIsm` for the remote domain on the `RoutingIsm`.
This is required for receiving messages from a remote (counterparty) chain.

```bash
celestia-appd tx hyperlane ism set-routing-ism-domain [routing-ism-id] [domain] [ism-id] [flags]
```

Example (Eden domain 714):
```bash
celestia-appd tx hyperlane ism set-routing-ism-domain \
  0x726f757465725f69736d000000000000000000000000000100000000000001c1 \
  714 \
  [0x726f757465725f69736d0000000000000000000000000004000000000000000a] \
  --from owner --fees 800utia
```

## Step 3: Set IGP Destination Gas Config
Add an IGP gas configuration for the remote chain, registered by its domain identifier.
This supports advance payments for gas fees on the destination.
This is required for sending messages to a remote (counterparty) chain.

```bash
celestia-appd tx hyperlane hooks igp set-destination-gas-config \
  [igp-id] \
  [remote-domain] \
  [token-exchange-rate] \
  [gas-price] \
  [gas-overhead] \
  [flags]
```