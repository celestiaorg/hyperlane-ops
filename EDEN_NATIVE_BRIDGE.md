# Eden Native Token Bridge to Celestia

This document describes the Hyperlane warp route that enables bridging native EDEN tokens from Eden Testnet to Celestia Mocha-4 Testnet.

## Overview

A bidirectional bridge has been deployed using Hyperlane's warp route technology, allowing users to:
- Send native EDEN tokens from Eden Testnet to Celestia (as synthetic EDEN)
- Send synthetic EDEN tokens from Celestia back to Eden (as native EDEN)

## Deployment Information

### Chain Details

**Eden Testnet**
- Domain ID: `2147483647`
- Chain ID: `3735928814`
- RPC: `https://ev-reth-eden-testnet.binarybuilders.services:8545`
- Mailbox: `0xBdEfA74aCf073Fc5c8961d76d5DdA87B1Be2C1b0`

**Celestia Mocha-4 Testnet**
- Domain ID: `1297040200`
- Chain ID: `mocha-4`
- RPC: `http://celestia-mocha-archive-rpc.mzonder.com:26657`
- Mailbox: `0x68797065726c616e650000000000000000000000000000000000000000000003`

### Deployed Contracts

**Eden Side (Native Token)**
- Contract Address: `0x954F1C87a6bc9d102CD4dC85e323500093f793ae`
- Type: Native (wraps native EDEN)
- Token: EDEN
- Decimals: 18

**Celestia Side (Synthetic Token)**
- Token ID: `0x726f757465725f61707000000000000000000000000000020000000000000007`
- Type: Synthetic (mints wrapped EDEN)
- Denom: `hyperlane/0x726f757465725f61707000000000000000000000000000020000000000000007`
- Decimals: 18

### Router Configuration

Both chains have been configured with remote routers:
- Eden → Celestia: Enrolled domain `1297040200` with router `0x726f757465725f61707000000000000000000000000000020000000000000007`
- Celestia → Eden: Enrolled domain `2147483647` with router `0x000000000000000000000000954F1C87a6bc9d102CD4dC85e323500093f793ae`

## How to Use the Bridge

### Sending EDEN from Eden to Celestia

1. **Get the recipient's Celestia address in hex format**

   Convert a Celestia bech32 address (e.g., `celestia1...`) to hex format with `0x` prefix and left-padded to 32 bytes.

2. **Call the `transferRemote` function**

   ```bash
   cast send 0x954F1C87a6bc9d102CD4dC85e323500093f793ae \
     "transferRemote(uint32,bytes32,uint256)" \
     1297040200 \
     <RECIPIENT_ADDRESS_IN_HEX> \
     <AMOUNT_IN_WEI> \
     --value <AMOUNT_IN_WEI> \
     --private-key <YOUR_PRIVATE_KEY> \
     --rpc-url https://ev-reth-eden-testnet.binarybuilders.services:8545
   ```

   **Parameters:**
   - `1297040200`: Celestia's domain ID
   - `<RECIPIENT_ADDRESS_IN_HEX>`: 32-byte hex address of recipient on Celestia
   - `<AMOUNT_IN_WEI>`: Amount to send in wei (18 decimals, e.g., `100000000000000000` = 0.1 EDEN)
   - `--value`: Must match the amount being sent (since this is a native token)

3. **Example: Send 0.1 EDEN**

   ```bash
   cast send 0x954F1C87a6bc9d102CD4dC85e323500093f793ae \
     "transferRemote(uint32,bytes32,uint256)" \
     1297040200 \
     0x000000000000000000000000FA1F92CEA15A8BF08156A857CCD71673CAD33FBC \
     100000000000000000 \
     --value 100000000000000000 \
     --private-key $YOUR_PRIVATE_KEY \
     --rpc-url https://ev-reth-eden-testnet.binarybuilders.services:8545
   ```

4. **Monitor the transfer**

   The Hyperlane relayer will automatically pick up your message and deliver it to Celestia within a few minutes.

### Checking Your Balance on Celestia

To check your synthetic EDEN balance on Celestia:

```bash
celestia-appd query bank balances <YOUR_CELESTIA_ADDRESS> \
  --node http://celestia-mocha-archive-rpc.mzonder.com:26657 \
  --output json | jq -r '.balances[] | select(.denom | contains("0x726f757465725f61707000000000000000000000000000020000000000000007"))'
```

**Example output:**
```json
{
  "denom": "hyperlane/0x726f757465725f61707000000000000000000000000000020000000000000007",
  "amount": "100000000000000000"
}
```

The amount is in base units with 18 decimals (e.g., `100000000000000000` = 0.1 EDEN).

### Sending EDEN from Celestia to Eden

To send synthetic EDEN tokens back from Celestia to Eden:

```bash
celestia-appd tx warp transfer \
  0x726f757465725f61707000000000000000000000000000020000000000000007 \
  2147483647 \
  <RECIPIENT_EVM_ADDRESS_IN_HEX> \
  <AMOUNT>hyperlane/0x726f757465725f61707000000000000000000000000000020000000000000007 \
  --from <YOUR_WALLET_NAME> \
  --node http://celestia-mocha-archive-rpc.mzonder.com:26657 \
  --chain-id mocha-4 \
  --fees 1000000utia \
  --gas 300000 \
  -y
```

**Parameters:**
- `0x726f757465725f61707000000000000000000000000000020000000000000007`: EDEN token ID on Celestia
- `2147483647`: Eden's domain ID
- `<RECIPIENT_EVM_ADDRESS_IN_HEX>`: Ethereum-style address on Eden (32 bytes, left-padded)
- `<AMOUNT>`: Amount with denomination (e.g., `100000000000000000hyperlane/0x726f757465725f61707000000000000000000000000000020000000000000007`)

## Technical Details

### Token Standard

- **Eden**: EvmHypNative (wraps native EDEN tokens)
- **Celestia**: CosmosNativeHypSynthetic (mints/burns synthetic representation)

### Gas Configuration

Router enrollments were configured with a gas limit of `1000000000` for Celestia → Eden transfers.

### Relayer

A Hyperlane relayer monitors both chains and automatically relays messages between them. The relayer configuration includes:
- Monitoring both Eden and Celestia chains
- Database path: `/tmp/hyperlane-relayer-db-eden`
- Metrics exposed on port `9091`

## Verification Commands

### Verify Router Enrollment on Eden

```bash
cast call 0x954F1C87a6bc9d102CD4dC85e323500093f793ae \
  "routers(uint32)(bytes32)" \
  1297040200 \
  --rpc-url https://ev-reth-eden-testnet.binarybuilders.services:8545
```

**Expected output:** `0x726f757465725f61707000000000000000000000000000020000000000000007`

### Verify Router Enrollment on Celestia

```bash
celestia-appd query warp remote-routers \
  0x726f757465725f61707000000000000000000000000000020000000000000007 \
  --node http://celestia-mocha-archive-rpc.mzonder.com:26657 \
  --output json
```

**Expected output:**
```json
{
  "remote_routers": [
    {
      "receiver_domain": 2147483647,
      "receiver_contract": "0x000000000000000000000000954F1C87a6bc9d102CD4dC85e323500093f793ae",
      "gas": "1000000000"
    }
  ]
}
```

### Get Token Details on Celestia

```bash
celestia-appd query warp token \
  0x726f757465725f61707000000000000000000000000000020000000000000007 \
  --node http://celestia-mocha-archive-rpc.mzonder.com:26657 \
  --output json
```

## Architecture

```
┌─────────────────────┐                    ┌──────────────────────┐
│   Eden Testnet      │                    │  Celestia Mocha-4    │
│                     │                    │                      │
│  Native EDEN Token  │◄──── Relayer ────►│  Synthetic EDEN      │
│  0x954F1C8...       │                    │  0x726f75746...      │
│                     │                    │                      │
│  Mailbox:           │                    │  Mailbox:            │
│  0xBdEfA74A...      │                    │  0x68797065...       │
└─────────────────────┘                    └──────────────────────┘
```

When a user sends EDEN from Eden:
1. Native EDEN is locked in the `HypNative` contract on Eden
2. A message is dispatched through the Hyperlane mailbox
3. The relayer picks up the message and submits it to Celestia
4. Synthetic EDEN tokens are minted to the recipient on Celestia

The reverse flow burns synthetic tokens on Celestia and unlocks native tokens on Eden.

## Support & Resources

- **Hyperlane Documentation**: https://docs.hyperlane.xyz
- **Eden Network**: https://edennetwork.io
- **Celestia Documentation**: https://docs.celestia.org

## Deployment Date

**Deployed:** October 22, 2025

## Status

✅ **Operational** - Bridge is live and tested with successful transfers.
