# Onboarding new chains to Celestia

This step-by-step guide serves as a walkthrough for onboarding a new Hyperlane chain to Celestia.
This assumes that all of the infrastructure has already been setup on the counterparty remote chain.

Generally, a three-step process is required on Celestia to fully onboard a new chain:

1. Create a new ism for verifying messages from a remote chain.
2. Register the new ism using the remote chain domain identifier in the Interchain Security Module (ISM) domain routing config.
3. Register a gas config using the remote chain domain identifier in the Interchain Gas Paymaster (IGP) destination gas configs.

!!! important
    This guide assumes operational knowledge of Celestia multisig accounts and transaction generation.
    Each of the transactions below can be output to JSON format using `--generate-only` and imported to <a>https://multisig.keplr.app/</a>.

## Step 1: Create Merkle Root Multisig ISM
Create the multisig ISM on Celestia with the validator addresses and threshold.

```bash
celestia-appd tx hyperlane ism create-merkle-root-multisig [validators] [threshold] [flags]
```

Example:

```bash
celestia-appd tx hyperlane ism create-merkle-root-multisig \
    0xdead..beef,0xabec..fe32,0xabee...00dd \ 
    2 \ 
    --signer multisig \
    --fees 800utia \
    --generate-only
```

!!! tip
    - The `validators` arg is a comma-separated list of `0x...` addresses.
    - The multisig ISM validator list must match the ECDSA checkpoint signing keys used by the validator agents.

## Step 2: Set Routing ISM Domain
Add the new `MerkleRootMultisigIsm` for the remote domain on the `RoutingIsm`.
This is required for receiving messages from a remote counterparty chain.

```bash
celestia-appd tx hyperlane ism set-routing-ism-domain [routing-ism-id] [domain] [ism-id] [flags]
```

Example:

```bash
celestia-appd tx hyperlane ism set-routing-ism-domain \
  0x726f757465725f69736d000000000000000000000000000100000000000001c1 \
  714 \
  0x726f757465725f69736d0000000000000000000000000004000000000000000a \
  --from multisig \
  --fees 800utia \ 
  --generate-only
```

## Step 3: Set IGP Destination Gas Config
Add an IGP gas configuration for the remote chain, registered by its domain identifier.
This supports advance payments for gas fees on the destination.
This is required for sending messages to a remote counterparty chain.

!!! important
    It may be that the IGP gas configs are updated by a trusted EOA to reduce friction when updating gas prices. TODO: This needs to be clarified!

```bash
celestia-appd tx hyperlane hooks igp set-destination-gas-config [igp-id] [remote-domain] [token-exchange-rate] [gas-price] [gas-overhead] [flags]
```

Example:

```bash
celestia-appd tx hyperlane hooks igp set-destination-gas-config \
  0x726f757465725f706f73745f6469737061746368000000040000000000000001 \
  1 \
  101 \
  300000000 \
  174289 \
  --from multisig \
  --fees 800utia \
  --generate-only
```