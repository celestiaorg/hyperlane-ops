# Cosmos Multisig ISM + IGP Setup

This page captures the current (partial) guidance for setting up a Cosmos multisig ISM and IGP destination gas config on Celestia. This document is an initial draft and work-in-progress which needs iteration. It should not be considered a full playbook yet.

## References
```
https://docs.hyperlane.xyz/docs/alt-vm-implementations/cosmos-sdk
https://hyperlanexyz.notion.site/Runbook-Hyperlane-Cosmos-SDK-2b06d35200d681f2a3c0e481a45b9275#2b06d35200d68177a982c87a8d4586cf
https://github.com/bcp-innovations/hyperlane-cosmos/tree/main
```

## High-Level Flow
1. Create a new Merkle root multisig ISM on Celestia that defines:
   - validator 0x addresses
   - threshold
2. Set the routing ISM domain mapping to point to the Eden ISM.
3. Set the IGP destination gas config for Eden.
4. Validators sign roots from the Eden mailbox; signatures are submitted to Celestia.
5. On-chain logic uses `ecrecover` to validate signatures against the ISM’s validator list. If threshold is met, the message is accepted.

## Step 0: Create Merkle Root Multisig ISM
Create the multisig ISM on Celestia with the validator addresses and threshold.

```
celestia-appd tx hyperlane ism create-merkle-root-multisig [validators] [threshold] [flags]
```

Example:

```
celestia-appd tx hyperlane ism create-merkle-root-multisig 0xdead..beef,0xabec..fe32,0xabee...00dd 2 --signer [signer] --fees 800utia
```

Notes:
- `validators` is a comma-separated list of 0x addresses.

Reference:
```
https://github.com/bcp-innovations/hyperlane-cosmos/blob/main/x/core/01_interchain_security/client/cli/tx.go#L114-L143
```

## Step 1: Set Routing ISM Domain
Use `celestia-appd` help docs to confirm flags.

```
celestia-appd tx hyperlane ism set-routing-ism-domain [routing-ism-id] [domain] [ism-id] [flags]
```

Example (Eden domain 714):
```
celestia-appd tx hyperlane ism set-routing-ism-domain \
  0x726f757465725f69736d000000000000000000000000000100000000000001c1 \
  714 \
  [THE_ISM_ID_FOR_EDEN_WHICH_HAS_TO_BE_SETUP] \
  --signer [signer] \
  --fees 800utia
```

## Step 2: Set IGP Destination Gas Config
Use `celestia-appd` help docs to confirm flags.

```
celestia-appd tx hyperlane hooks igp set-destination-gas-config \
  [igp-id] \
  [remote-domain] \
  [token-exchange-rate] \
  [gas-price] \
  [gas-overhead] \
  [flags]
```

Known values:
```
igp-id = 0x726f757465725f706f73745f6469737061746368000000040000000000000001
remote-domain = 714  # Eden
```

Unknown values:
- `token-exchange-rate`
- `gas-price`
- `gas-overhead`

## Required Inputs (Still Missing)
- ISM ID for Eden (created on Celestia).
- Validator 0x addresses for multisig participants.
- Threshold for multisig ISM.
- IGP gas config values (token exchange rate, gas price, gas overhead).

## Notes / Gaps
- Validators do not need to announce themselves before Step 1, but their 0x addresses are required to create the ISM.
- The `create-merkle-root-multisig` command exists in `celestia-appd`; confirm exact flags using the CLI help output.
- The only current doc for step-by-step CLI usage is the `celestia-appd` help output.
