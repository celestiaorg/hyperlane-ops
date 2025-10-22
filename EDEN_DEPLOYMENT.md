# Eden-Celestia Warp Route Deployment

## Overview
This document tracks the deployment of a Hyperlane warp route between Eden Testnet and Celestia Mocha-4 Testnet, using the existing Celestia mailbox infrastructure.

## Deployment Summary

### Chain Information

**Eden Testnet**
- Domain ID: `2147483647`
- Chain ID: `3735928814`
- RPC: `https://ev-reth-eden-testnet.binarybuilders.services:8545`
- Mailbox: `0xBdEfA74aCf073Fc5c8961d76d5DdA87B1Be2C1b0` (pre-existing)
- MerkleTreeHook: `0x22379102569dc3fBeA23Dc34e27F52a76c60F034` (pre-existing)

**Celestia Mocha-4 Testnet**
- Domain ID: `1297040200`
- Chain ID: `mocha-4`
- RPC: `http://celestia-mocha-archive-rpc.mzonder.com:26657`
- Mailbox: `0x68797065726c616e650000000000000000000000000000000000000000000003` (pre-existing)
- MerkleTreeHook: `0x726f757465725f706f73745f6469737061746368000000030000000000000005` (pre-existing)

### Deployed Warp Route

**Celestia Side (Collateral Token)**
- Token ID: `0x726f757465725f61707000000000000000000000000000010000000000000006`
- Type: Collateral (locks native TIA)
- Creation TX: `B7CA889507C23F3863D43817EFE8D878A67A07D80FF7C5063BEBB87449E88338`

**Eden Side (Synthetic Token)**
- Contract Address: `0xcC79A8081D8cFd3da61dE7a25Bc06CCe00BbDa54`
- Type: Synthetic (mints wrapped TIA)
- Symbol: TIA
- Decimals: 6
- ProxyAdmin: `0x2c58a988d6eAE104BFdbe61c8f3271a483311e4f`
- Deployment TX: `0x7974929b44692bfa146ae12d6bd4b16f301f66b3ae9a4c37a5636096b1d89615`
- Gas Used: 0.00464236003249652 ETH

### Router Enrollment

**Celestia → Eden**
- Transaction: `6E2A53FC03D8D3016407A9B5DED65EC21DC9312A2D2C98A066D0F8F8771E84E6`
- Remote Domain: `2147483647` (Eden)
- Remote Router: `0x000000000000000000000000cC79A8081D8cFd3da61dE7a25Bc06CCe00BbDa54`
- Gas Limit: `1000000000`

**Eden → Celestia**
- Transaction: `0x651b52602ff33c77ed34084ece453419f2efe88858dfba678c6825f8ff0473c8`
- Remote Domain: `1297040200` (Celestia)
- Remote Router: `0x726f757465725f61707000000000000000000000000000010000000000000006`
- Gas Used: 120235

## Deployment Steps Executed

1. ✅ Created collateral token on Celestia using existing mailbox
   ```bash
   cd ../celestia-app
   ./build/celestia-appd tx warp create-collateral-token \
     0x68797065726c616e650000000000000000000000000000000000000000000003 \
     utia \
     --from relayer-wallet \
     --node http://celestia-mocha-archive-rpc.mzonder.com:26657 \
     --chain-id mocha-4 \
     --fees 1000000utia \
     --gas 300000 \
     -y
   ```

2. ✅ Deployed synthetic TIA token on Eden testnet
   ```bash
   source .env
   hyperlane warp deploy \
     --registry ./registry \
     --config ./configs/eden-mocha-new-warp-config.yaml \
     --yes
   ```

3. ✅ Enrolled remote routers bidirectionally
   - Celestia side: enrolled Eden router for domain 2147483647
   - Eden side: enrolled Celestia router for domain 1297040200

4. ✅ Created relayer configuration
   - Config file: `config/relayer-config-eden-mocha.json`
   - Run script: `run-relayer-eden.sh`
   - Database: `/tmp/hyperlane-relayer-db-eden`
   - Metrics port: 9091

## Running the Relayer

### Prerequisites

1. Build the relayer from hyperlane-monorepo:
   ```bash
   cd ../hyperlane-monorepo/rust/main
   cargo build --release --bin relayer
   ```

2. Ensure signer keys are configured in `.env`:
   ```bash
   export HYP_CHAINS_EDENTESTNET_SIGNER_KEY=<YOUR_EDEN_PRIVATE_KEY>
   export HYP_CHAINS_CELESTIATESTNET_SIGNER_KEY=<YOUR_CELESTIA_PRIVATE_KEY>
   ```

### Start the Relayer

```bash
./run-relayer-eden.sh
```

The relayer will:
- Monitor both Eden and Celestia chains
- Relay messages between the chains
- Expose metrics on port 9091

## Testing the Warp Route

### Transfer TIA from Celestia to Eden

1. Check initial balances:
   ```bash
   # Celestia TIA balance
   cd ../celestia-app
   ./build/celestia-appd query bank balances celestia1lg0e9n4pt29lpq2k4ptue4ckw09dx0aujlpe4j \
     --node http://celestia-mocha-archive-rpc.mzonder.com:26657 \
     --output json | jq -r '.balances[] | select(.denom=="utia")'

   # Eden synthetic TIA balance
   cast call 0xcC79A8081D8cFd3da61dE7a25Bc06CCe00BbDa54 \
     "balanceOf(address)(uint256)" \
     0xc259e540167B7487A89b45343F4044d5951cf871 \
     --rpc-url https://ev-reth-eden-testnet.binarybuilders.services:8545
   ```

2. Initiate transfer from Celestia:
   ```bash
   cd ../celestia-app
   ./build/celestia-appd tx warp transfer \
     0x726f757465725f61707000000000000000000000000000010000000000000006 \
     2147483647 \
     0x000000000000000000000000c259e540167B7487A89b45343F4044d5951cf871 \
     1000000utia \
     --from relayer-wallet \
     --node http://celestia-mocha-archive-rpc.mzonder.com:26657 \
     --chain-id mocha-4 \
     --fees 1000000utia \
     --gas 300000 \
     -y
   ```

3. Monitor relayer logs for message processing

4. Verify receipt on Eden:
   ```bash
   cast call 0xcC79A8081D8cFd3da61dE7a25Bc06CCe00BbDa54 \
     "balanceOf(address)(uint256)" \
     0xc259e540167B7487A89b45343F4044d5951cf871 \
     --rpc-url https://ev-reth-eden-testnet.binarybuilders.services:8545
   ```

### Transfer TIA from Eden to Celestia

1. Initiate transfer from Eden:
   ```bash
   cast send 0xcC79A8081D8cFd3da61dE7a25Bc06CCe00BbDa54 \
     "transferRemote(uint32,bytes32,uint256)" \
     1297040200 \
     0x000000000000000000000000FA1F92CEA15A8BF08156A857CCD71673CAD33FBC \
     1000000 \
     --private-key $HYP_KEY \
     --rpc-url https://ev-reth-eden-testnet.binarybuilders.services:8545
   ```

2. Monitor relayer and verify receipt on Celestia

## Key Differences from XO Market Deployment

1. **Reused existing Celestia mailbox**: Instead of deploying new core contracts, we created a new collateral token using the existing mailbox infrastructure (mailbox ID: `0x68797065726c616e650000000000000000000000000000000000000000000003`)

2. **Different domain ID**: Eden uses domain `2147483647` vs XO Market's `1000101`

3. **Separate relayer database**: Using `/tmp/hyperlane-relayer-db-eden` to avoid conflicts with the XO Market relayer

4. **Different metrics port**: Using port 9091 instead of 9090

## Verification Commands

### Verify Router Enrollment

```bash
# Check Eden side
cast call 0xcC79A8081D8cFd3da61dE7a25Bc06CCe00BbDa54 \
  "routers(uint32)(bytes32)" \
  1297040200 \
  --rpc-url https://ev-reth-eden-testnet.binarybuilders.services:8545

# Check Celestia side
cd ../celestia-app
./build/celestia-appd query warp remote-routers \
  0x726f757465725f61707000000000000000000000000000010000000000000006 \
  --node http://celestia-mocha-archive-rpc.mzonder.com:26657 \
  --output json
```

### Check MerkleTreeHook Status

```bash
# Eden
cast call 0x22379102569dc3fBeA23Dc34e27F52a76c60F034 \
  "count()(uint32)" \
  --rpc-url https://ev-reth-eden-testnet.binarybuilders.services:8545

# Celestia
cd ../celestia-app
./build/celestia-appd query hyperlane hooks merkle-tree-hook \
  0x726f757465725f706f73745f6469737061746368000000030000000000000005 \
  --node http://celestia-mocha-archive-rpc.mzonder.com:26657 \
  --output json
```

## Notes

- This deployment reuses the same Celestia mailbox that was used for the XO Market deployment
- Multiple warp routes can share the same mailbox infrastructure
- Each collateral token gets a unique token ID on Celestia
- The relayer can handle multiple routes by running multiple instances with different configs
- Eden testnet had pre-existing Hyperlane core infrastructure with 16 messages already processed

## Configuration Files

- Warp config: `configs/eden-mocha-new-warp-config.yaml`
- Relayer config: `config/relayer-config-eden-mocha.json`
- Run script: `run-relayer-eden.sh`

---

**Deployment Date**: 2025-10-22
**Deployed By**: Using keys from .env and .secret
**Status**: ✅ Complete and ready for testing
