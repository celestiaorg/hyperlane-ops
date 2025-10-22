# Hyperlane Bridge: XO Market ↔ Celestia Testnet

Quick deployment guide for bidirectional token bridging between XO Market Testnet and Celestia Mocha-4.

## What Was Deployed

Two bidirectional warp routes:

1. **TIA Bridge**: Native TIA (Celestia) ↔ Synthetic TIA (XO Market)
2. **XO Bridge**: Native XO (XO Market) ↔ Synthetic XO (Celestia)

### Deployed Addresses

#### XO Market Testnet
- **Mailbox**: `0x8ED282d598296A4FCb460CBe6115446c0Dc3FD3E`
- **MerkleTreeHook**: `0xb58a0742AA0986eC81D28356E07612Cf23bA95b9`
- **Synthetic TIA Token**: `0x1d853F9d19c1F93B32149e99bD0c3A45E681CBc3`
- **Native XO Warp**: `0x8Fe2521c2DAbB53c176fFBb73C3083eC319454Ac`

#### Celestia Mocha-4
- **Mailbox**: `0x68797065726c616e650000000000000000000000000000000000000000000003`
- **MerkleTreeHook**: `0x726f757465725f706f73745f6469737061746368000000030000000000000005`
- **Collateral TIA**: `0x726f757465725f61707000000000000000000000000000010000000000000004`
- **Synthetic XO**: `0x726f757465725f61707000000000000000000000000000020000000000000005`

---

## Quick Start: Reproduce Deployment

### Prerequisites
```bash
# Install Hyperlane CLI
npm install -g @hyperlane-xyz/cli

# Install Foundry (for cast)
curl -L https://foundry.paradigm.xyz | bash && foundryup

# Tools needed:
# - hyp (for Celestia NoopISM deployment)
# - celestia-appd (for Celestia warp routes)
# - Hyperlane relayer (Rust binary)
```

### Step 1: Deploy on EVM Chain (XO Market)

```bash
# 1. Create registry metadata
mkdir -p ./registry/chains/xomarkettestnet
cat > ./registry/chains/xomarkettestnet/metadata.yaml <<EOF
chainId: 1000101
domainId: 1000101
protocol: ethereum
name: xomarkettestnet
nativeToken:
  decimals: 18
  name: XO
  symbol: XO
rpcUrls:
  - http: https://testnet-rpc-1.xo.market
EOF

# 2. Create core config with MerkleTreeHook
cat > ./configs/core-config.yaml <<EOF
owner: "0xYOUR_ADDRESS"
defaultIsm:
  type: trustedRelayerIsm
  relayer: "0xYOUR_ADDRESS"
defaultHook:
  type: merkleTreeHook
requiredHook:
  type: protocolFee
  beneficiary: "0xYOUR_ADDRESS"
  owner: "0xYOUR_ADDRESS"
  protocolFee: "0"
EOF

# 3. Deploy core contracts
hyperlane core deploy \
  --registry ./registry \
  --config ./configs/core-config.yaml \
  --chain xomarkettestnet \
  --key 0xYOUR_PRIVATE_KEY

# Save the Mailbox and MerkleTreeHook addresses!
```

### Step 2: Deploy on Celestia

```bash
# 1. Set environment
export HYP_MNEMONIC="your mnemonic here"
export HYP_CHAIN_ID="mocha-4"

# 2. Deploy NoopISM stack
./hyp deploy-noopism public-celestia-mocha4-consensus.numia.xyz:9090

# 3. IMPORTANT: Create MerkleTreeHook manually
celestia-appd tx hyperlane hooks merkle create \
  <MAILBOX_ID> \
  --from your-wallet \
  --node http://celestia-mocha-archive-rpc.mzonder.com:26657 \
  --chain-id mocha-4 \
  --fees 1000000utia \
  -y

# Query the tx to get MerkleTreeHook ID - you'll need this!
```

### Step 3: Deploy Warp Routes

**For TIA (Celestia → XO Market):**

```bash
# On Celestia: Create collateral token
celestia-appd tx warp create-collateral-token <MAILBOX_ID> utia \
  --from your-wallet --node <RPC> --chain-id mocha-4 --fees 1000000utia -y

# On XO Market: Deploy synthetic token
cat > ./configs/tia-warp-config.yaml <<EOF
xomarkettestnet:
  type: synthetic
  owner: "0xYOUR_ADDRESS"
  mailbox: "<MAILBOX_ADDRESS>"
  name: "TIA"
  symbol: "TIA"
  decimals: 6
EOF

hyperlane warp deploy --config ./configs/tia-warp-config.yaml --key 0xYOUR_KEY
```

**For XO (XO Market → Celestia):**

```bash
# On XO Market: Deploy native warp
cat > ./configs/xo-warp-config.yaml <<EOF
xomarkettestnet:
  type: native
  owner: "0xYOUR_ADDRESS"
  mailbox: "<MAILBOX_ADDRESS>"
EOF

hyperlane warp deploy --config ./configs/xo-warp-config.yaml --key 0xYOUR_KEY

# On Celestia: Create synthetic token
celestia-appd tx warp create-synthetic-token <MAILBOX_ID> \
  --from your-wallet --node <RPC> --chain-id mocha-4 --fees 1000000utia -y
```

### Step 4: Enroll Routers (Bidirectional)

**Enroll on Celestia:**
```bash
celestia-appd tx warp enroll-remote-router \
  <TOKEN_ID> \
  <REMOTE_DOMAIN> \
  <REMOTE_ROUTER_ADDRESS_32_BYTES> \
  1000000000 \
  --from your-wallet --node <RPC> --chain-id mocha-4 --fees 1000000utia -y
```

**Enroll on XO Market:**
```bash
cast send <WARP_TOKEN_ADDRESS> \
  "enrollRemoteRouter(uint32,bytes32)" \
  <REMOTE_DOMAIN> \
  <REMOTE_ROUTER_ID> \
  --private-key 0xYOUR_KEY \
  --rpc-url https://testnet-rpc-1.xo.market
```

### Step 5: Configure Relayer

```bash
# 1. Create config (see DEPLOYMENT_GUIDE.md for full config)
# Key points:
# - Set correct merkleTreeHook addresses for BOTH chains
# - Use archive RPC for Celestia
# - Set index.from to block before first deployment

# 2. Set environment variables
export HYP_CHAINS_XOMARKETTESTNET_SIGNER_KEY=0xYOUR_KEY
export HYP_CHAINS_CELESTIATESTNET_SIGNER_KEY=0xYOUR_KEY
export HYP_BASE_DB=/tmp/hyperlane-relayer-db

# 3. Run relayer
./relayer
```

---

## How to Transfer Tokens

### Send TIA from Celestia to XO Market

```bash
# Convert EVM address to 32-byte format
# 0xYourAddress → 0x000000000000000000000000YourAddress

celestia-appd tx warp transfer \
  <TIA_TOKEN_ID> \
  1000101 \
  0x000000000000000000000000<EVM_ADDRESS> \
  1000000utia \
  --from your-wallet \
  --node http://celestia-mocha-archive-rpc.mzonder.com:26657 \
  --chain-id mocha-4 \
  --fees 1000000utia \
  -y
```

### Send XO from XO Market to Celestia

```bash
# Convert Celestia bech32 address to hex
celestia-appd debug addr celestia1yourbech32address
# Pad to 32 bytes: 0x000000000000000000000000<HEX_ADDRESS>

cast send <XO_WARP_TOKEN> \
  "transferRemote(uint32,bytes32,uint256)" \
  1297040200 \
  0x000000000000000000000000<CELESTIA_HEX_ADDRESS> \
  10000000000000000 \
  --value 0.01ether \
  --private-key 0xYOUR_KEY \
  --rpc-url https://testnet-rpc-1.xo.market
```

---

## Verification

### Check Balances

**Celestia:**
```bash
# Native TIA
celestia-appd query bank balances <ADDRESS> --node <RPC> --output json

# Synthetic XO (denom contains the token ID)
# Look for: hyperlane/0x726f757465725f61707000000000000000000000000000020000000000000005
```

**XO Market:**
```bash
# Native XO
cast balance <ADDRESS> --rpc-url https://testnet-rpc-1.xo.market --ether

# Synthetic TIA
cast call 0x1d853F9d19c1F93B32149e99bD0c3A45E681CBc3 \
  "balanceOf(address)(uint256)" <ADDRESS> \
  --rpc-url https://testnet-rpc-1.xo.market
```

### Verify Routers Are Enrolled

**Celestia:**
```bash
celestia-appd query warp remote-routers <TOKEN_ID> --node <RPC> --output json
```

**XO Market:**
```bash
cast call <WARP_TOKEN> "routers(uint32)(bytes32)" <REMOTE_DOMAIN> --rpc-url <RPC>
```

---

## Successful Test Transactions

### TIA Transfer: Celestia → XO Market
- **TX Hash**: `D3DBCEC059E0E99A0FCF802D8F4821372A43E0831F939D81408E045EC300E6EC`
- **Amount**: 1 TIA
- **Result**: ✅ Synthetic TIA minted on XO Market

### TIA Return: XO Market → Celestia
- **TX Hash**: `0xcc99afe3bc5c98c3ea5195cf86b908500975d0542d27742c7172f02ccada9f4c`
- **Amount**: 0.5 TIA
- **Result**: ✅ Native TIA released on Celestia

### XO Transfer: XO Market → Celestia
- **TX Hash**: `0xd76447420f794e7d6f672f097713eac7d480f2d5d8c0c6b3a39464f0f014c4c8`
- **Amount**: 0.01 XO
- **Result**: ✅ Synthetic XO minted on Celestia

---

## Critical Notes

### ⚠️ MerkleTreeHook is MANDATORY

**Without MerkleTreeHook:**
- ❌ Relayer fails with "Failed to query sequence"
- ❌ Messages are never relayed
- ❌ Bridge doesn't work

**Always ensure:**
- EVM: Deploy core with `defaultHook: type: merkleTreeHook`
- Celestia: Create MerkleTreeHook manually after NoopISM deployment
- Relayer config has correct `merkleTreeHook` addresses for BOTH chains

### Router Enrollment Must Be Bidirectional

Both chains must know about each other:
- Celestia token → knows about XO Market router
- XO Market token → knows about Celestia token

### Address Format Matters

- **EVM → Cosmos**: Pad EVM address to 32 bytes
  - `0xc259...` → `0x000000000000000000000000c259...`
- **Cosmos → EVM**: Convert bech32 to hex, then pad
  - `celestia1lg0...` → hex → `0x000000000000000000000000FA1F...`

### Relayer Requirements

1. **Archive RPC for Celestia** (must have historical blocks)
2. **Correct signer keys** for both chains
3. **Funded addresses** for gas on both chains
4. **Clean database** when switching between deployments

---

## Troubleshooting

### "Failed to query sequence"
- **Fix**: Verify MerkleTreeHook exists and is in relayer config
- See DEPLOYMENT_GUIDE.md section 2.3

### "Sequence count mismatch"
- **Fix**: Clear relayer database and restart
  ```bash
  rm -rf /tmp/hyperlane-relayer-db
  ```

### Messages not being relayed
- Check routers are enrolled (both directions)
- Verify relayer is synced from correct block
- Check relayer logs for errors

---

## Resources

- **Full Documentation**: See `DEPLOYMENT_GUIDE.md` for detailed explanations
- **Hyperlane Docs**: https://docs.hyperlane.xyz
- **XO Market RPC**: https://testnet-rpc-1.xo.market
- **Celestia Archive RPC**: http://celestia-mocha-archive-rpc.mzonder.com:26657

## Chain Information

- **XO Market Domain ID**: 1000101
- **Celestia Domain ID**: 1297040200
- **Celestia Chain ID**: mocha-4

---

**Status**: ✅ Fully operational, tested with successful bidirectional transfers
**Date**: October 21, 2025
