# Hyperlane Warp Route Deployment Guide

Complete guide for deploying bidirectional Hyperlane warp routes between Celestia (Cosmos) and EVM chains.

## Table of Contents

1. [Overview](#overview)
2. [Prerequisites](#prerequisites)
3. [Step 1: Chain Registry Setup](#step-1-chain-registry-setup)
4. [Step 2: Deploy Hyperlane Core on EVM Chain](#step-2-deploy-hyperlane-core-on-evm-chain)
5. [Step 3: Deploy Hyperlane Core on Celestia](#step-3-deploy-hyperlane-core-on-celestia)
6. [Step 4: Deploy Warp Routes](#step-4-deploy-warp-routes)
7. [Step 5: Enroll Remote Routers](#step-5-enroll-remote-routers)
8. [Step 6: Configure and Run Relayer](#step-6-configure-and-run-relayer)
9. [Step 7: Testing Transfers](#step-7-testing-transfers)
10. [Troubleshooting](#troubleshooting)
11. [Example Transactions](#example-transactions)

---

## Overview

This guide demonstrates deploying bidirectional warp routes between:
- **Celestia Mocha-4 Testnet** (Cosmos chain)
- **XO Market Testnet** (EVM chain)

We deployed two warp routes:
1. **TIA Route**: Native TIA on Celestia ↔ Synthetic TIA on XO Market
2. **XO Route**: Native XO on XO Market ↔ Synthetic XO on Celestia

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                   Hyperlane Relayer                          │
│          (Monitors both chains, relays messages)             │
└─────────────────────────────────────────────────────────────┘
                    ↓                    ↓
    ┌───────────────────────┐   ┌───────────────────────┐
    │  Celestia Mocha-4     │   │  XO Market Testnet    │
    │  (Cosmos Chain)       │   │  (EVM Chain)          │
    ├───────────────────────┤   ├───────────────────────┤
    │ Mailbox               │   │ Mailbox               │
    │ MerkleTreeHook        │   │ MerkleTreeHook        │
    │ Collateral TIA Token  │   │ Synthetic TIA Token   │
    │ Synthetic XO Token    │   │ Native XO Token       │
    └───────────────────────┘   └───────────────────────┘
```

---

## Prerequisites

### Required Tools

1. **Hyperlane CLI** (for EVM deployments)
   ```bash
   npm install -g @hyperlane-xyz/cli
   ```

2. **hyp tool** (for Celestia NoopISM deployments)
   - Custom Go-based CLI from celestia-zkevm-hl-testnet
   - Binary location: `../celestia-zkevm-hl-testnet/hyperlane/hyp`

3. **celestia-appd** (for Celestia warp routes)
   - Build from celestia-app repository
   - Binary location: `../celestia-app/build/celestia-appd`

4. **Hyperlane Relayer** (Rust binary)
   - Build from hyperlane-monorepo
   - Binary location: `../hyperlane-monorepo/rust/main/target/release/relayer`

5. **Foundry** (for EVM interactions)
   ```bash
   curl -L https://foundry.paradigm.xyz | bash
   foundryup
   ```

### Required Information

- **Private keys/mnemonics** for both chains
- **RPC endpoints** for both chains
- **Sufficient native tokens** for gas fees on both chains

---

## Step 1: Chain Registry Setup

Create chain metadata files for Hyperlane CLI to recognize your chains.

### 1.1 Celestia Testnet Registry

Create `./registry/chains/celestiatestnet/metadata.yaml`:

```yaml
chainId: mocha-4
displayName: Celestia Mocha Testnet
domainId: 1297040200
protocol: cosmosnative
name: celestiatestnet
nativeToken:
  decimals: 6
  denom: utia
  name: TIA
  symbol: TIA
rpcUrls:
  - http: http://celestia-mocha-archive-rpc.mzonder.com:26657
restUrls:
  - http: https://api.celestia-mocha.com
grpcUrls:
  - http: http://public-celestia-mocha4-consensus.numia.xyz:9090
gasPrice:
  amount: "0.1"
  denom: utia
bech32Prefix: celestia
slip44: 118
```

**Key Points:**
- `domainId`: Unique identifier for Hyperlane (must match deployment configs)
- `protocol: cosmosnative`: Indicates Cosmos SDK chain
- Archive RPC required for relayer to sync historical messages
- gRPC endpoint needed for Cosmos queries

### 1.2 XO Market Testnet Registry

Create `./registry/chains/xomarkettestnet/metadata.yaml`:

```yaml
chainId: 1000101
displayName: XO Market Testnet
domainId: 1000101
protocol: ethereum
name: xomarkettestnet
nativeToken:
  decimals: 18
  name: XO
  symbol: XO
rpcUrls:
  - http: https://testnet-rpc-1.xo.market
blocks:
  confirmations: 1
  reorgPeriod: 1
  estimateBlockTime: 12
```

**Key Points:**
- `domainId` and `chainId` are the same for EVM chains
- `protocol: ethereum`: Standard for EVM-compatible chains
- `blocks.confirmations`: Number of confirmations before considering finalized

---

## Step 2: Deploy Hyperlane Core on EVM Chain

Deploy the core Hyperlane contracts (mailbox, ISM, hooks) on the EVM chain using Hyperlane CLI.

### 2.1 Create Core Configuration

Create `./configs/core-config.yaml`:

```yaml
owner: "0xc259e540167B7487A89b45343F4044d5951cf871"
defaultIsm:
  type: trustedRelayerIsm
  relayer: "0xc259e540167B7487A89b45343F4044d5951cf871"
defaultHook:
  type: merkleTreeHook
requiredHook:
  type: protocolFee
  beneficiary: "0xc259e540167B7487A89b45343F4044d5951cf871"
  maxProtocolFee: "10000000000000000000000000000"
  owner: "0xc259e540167B7487A89b45343F4044d5951cf871"
  protocolFee: "0"
```

**Configuration Explained:**
- `owner`: Address that controls the contracts
- `defaultIsm`: Interchain Security Module (trustedRelayerIsm for testing)
- `defaultHook`: **CRITICAL - Must be merkleTreeHook** (see section 2.3)
- `requiredHook`: Protocol fee hook (set to 0 for testing)

### 2.2 Deploy Core Contracts

```bash
hyperlane core deploy \
  --registry ./registry \
  --config ./configs/core-config.yaml \
  --chain xomarkettestnet \
  --key 0xYOUR_PRIVATE_KEY
```

**Output Example:**
```
Mailbox: 0x8ED282d598296A4FCb460CBe6115446c0Dc3FD3E
MerkleTreeHook: 0xb58a0742AA0986eC81D28356E07612Cf23bA95b9
ValidatorAnnounce: 0xa542A7DD8BAE55f35614224eCF9E287c73656F00
ProxyAdmin: 0x17Dc7Ff9592dCd3049B53B3558039EaCa56eFe2d
```

**Save these addresses** - you'll need them for warp route deployment and relayer configuration.

### 2.3 Why MerkleTreeHook is Critical

**The MerkleTreeHook is ESSENTIAL for the relayer to function properly.**

#### What is MerkleTreeHook?

The MerkleTreeHook maintains a Merkle tree of all outgoing messages. Each message gets:
1. A unique sequence number (incrementing counter)
2. A position in the Merkle tree
3. A Merkle proof for verification

#### What Happens WITHOUT MerkleTreeHook?

If you use `NoopHook` or no hook:
- ❌ Messages are dispatched but not tracked
- ❌ Relayer cannot query message count
- ❌ Relayer cannot build Merkle proofs
- ❌ **Relayer fails with "Failed to query sequence" error**
- ❌ Messages are never relayed

#### Symptoms of Missing MerkleTreeHook:

```bash
# Relayer logs will show:
WARN hyperlane_core::rpc_clients::retry: Retrying call,
  error: EyreError(Failed to query sequence)

# The relayer cannot determine how many messages exist
# It keeps retrying and eventually crashes
```

#### How to Verify MerkleTreeHook:

```bash
# Check mailbox's default hook
cast call <MAILBOX_ADDRESS> "defaultHook()(address)" --rpc-url <RPC_URL>

# Check MerkleTreeHook message count
cast call <MERKLE_TREE_HOOK_ADDRESS> "count()(uint32)" --rpc-url <RPC_URL>
```

---

## Step 3: Deploy Hyperlane Core on Celestia

Celestia uses a different deployment process because it's a Cosmos chain, not EVM.

### 3.1 Using the `hyp` Tool for NoopISM Deployment

The `hyp` tool deploys a complete Hyperlane stack on Celestia with NoopISM (no-operation ISM for testing).

#### Environment Setup

```bash
export HYP_MNEMONIC="your twelve word mnemonic phrase here"
export HYP_CHAIN_ID="mocha-4"
```

**Important:**
- `hyp` tool requires `HYP_MNEMONIC`, not `HYP_KEY`
- The mnemonic should correspond to a funded Celestia account

#### Deploy NoopISM Stack

```bash
../celestia-zkevm-hl-testnet/hyperlane/hyp deploy-noopism \
  public-celestia-mocha4-consensus.numia.xyz:9090
```

**What This Deploys:**
- Mailbox (hyperlane module)
- NoopISM (for testing, accepts all messages)
- **NoopHook** (NOT MerkleTreeHook - this is a problem!)

**Output:**
```
Mailbox ID: 0x68797065726c616e650000000000000000000000000000000000000000000003
NoopHook ID: 0x726f757465725f706f73745f6469737061746368000000000000000000000004
```

### 3.2 Creating MerkleTreeHook Manually (CRITICAL)

**The NoopISM deployment does NOT include a MerkleTreeHook**, which breaks the relayer.

#### Why We Need to Create It:

The relayer requires a MerkleTreeHook to:
1. Query the total number of dispatched messages
2. Track which messages have been indexed
3. Build Merkle proofs for message verification on the destination chain

#### Create MerkleTreeHook:

```bash
cd ../celestia-app

./build/celestia-appd tx hyperlane hooks merkle create \
  0x68797065726c616e650000000000000000000000000000000000000000000003 \
  --from relayer-wallet \
  --node http://celestia-mocha-archive-rpc.mzonder.com:26657 \
  --chain-id mocha-4 \
  --fees 1000000utia \
  --gas 300000 \
  -y
```

**Parameters:**
- First argument: Mailbox ID
- `--from`: Your Celestia wallet name
- `--fees`: Transaction fees in utia

#### Query the Transaction to Get Hook ID:

```bash
./build/celestia-appd query tx <TX_HASH> \
  --node http://celestia-mocha-archive-rpc.mzonder.com:26657 \
  --output json | jq -r '.events[] | select(.type=="hyperlane.core.post_dispatch.v1.EventCreatedMerkleTreeHook") | .attributes[] | select(.key=="merkle_tree_hook_id") | .value'
```

**Expected Output:**
```
0x726f757465725f706f73745f6469737061746368000000030000000000000005
```

**Save this MerkleTreeHook ID** - you'll need it for:
1. Relayer configuration
2. Warp route deployments

#### Verify Hook Creation:

```bash
./build/celestia-appd query hyperlane hooks merkle-tree-hook \
  0x726f757465725f706f73745f6469737061746368000000030000000000000005 \
  --node http://celestia-mocha-archive-rpc.mzonder.com:26657 \
  --output json
```

---

## Step 4: Deploy Warp Routes

Now deploy the token warp routes. We'll deploy two routes:
1. TIA (native on Celestia → synthetic on XO Market)
2. XO (native on XO Market → synthetic on Celestia)

### 4.1 TIA Warp Route (Celestia → XO Market)

#### Step 1: Create Collateral Token on Celestia

First, create the collateral token that will lock native TIA:

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

**Parameters:**
- First argument: Mailbox ID
- Second argument: Native token denom (`utia` for TIA)

**Query transaction to get Token ID:**

```bash
./build/celestia-appd query tx <TX_HASH> \
  --node http://celestia-mocha-archive-rpc.mzonder.com:26657 \
  --output json | jq -r '.events[] | select(.type=="hyperlane.warp.v1.EventCreateCollateralToken") | .attributes[] | select(.key=="token_id") | .value'
```

**Example Output:**
```
0x726f757465725f61707000000000000000000000000000010000000000000004
```

#### Step 2: Deploy Synthetic Token on XO Market

Create config file `./configs/xomarket-mocha-warp-config.yaml`:

```yaml
xomarkettestnet:
  type: synthetic
  owner: "0xc259e540167B7487A89b45343F4044d5951cf871"
  mailbox: "0x8ED282d598296A4FCb460CBe6115446c0Dc3FD3E"
  name: "TIA"
  symbol: "TIA"
  decimals: 6
```

**Deploy using Hyperlane CLI:**

```bash
hyperlane warp deploy \
  --registry ./registry \
  --config ./configs/xomarket-mocha-warp-config.yaml \
  --key 0xYOUR_PRIVATE_KEY
```

**Output:**
```
Synthetic TIA Token: 0x1d853F9d19c1F93B32149e99bD0c3A45E681CBc3
```

### 4.2 XO Warp Route (XO Market → Celestia)

#### Step 1: Deploy Native Token on XO Market

Create config file `./configs/xo-native-warp-config.yaml`:

```yaml
xomarkettestnet:
  type: native
  owner: "0xc259e540167B7487A89b45343F4044d5951cf871"
  mailbox: "0x8ED282d598296A4FCb460CBe6115446c0Dc3FD3E"
```

**Deploy using Hyperlane CLI:**

```bash
hyperlane warp deploy \
  --registry ./registry \
  --config ./configs/xo-native-warp-config.yaml \
  --key 0xYOUR_PRIVATE_KEY
```

**Output:**
```
Native XO Warp Token: 0x8Fe2521c2DAbB53c176fFBb73C3083eC319454Ac
```

#### Step 2: Create Synthetic Token on Celestia

```bash
cd ../celestia-app

./build/celestia-appd tx warp create-synthetic-token \
  0x68797065726c616e650000000000000000000000000000000000000000000003 \
  --from relayer-wallet \
  --node http://celestia-mocha-archive-rpc.mzonder.com:26657 \
  --chain-id mocha-4 \
  --fees 1000000utia \
  --gas 300000 \
  -y
```

**Query transaction to get Token ID:**

```bash
./build/celestia-appd query tx <TX_HASH> \
  --node http://celestia-mocha-archive-rpc.mzonder.com:26657 \
  --output json | jq -r '.events[] | select(.type=="hyperlane.warp.v1.EventCreateSyntheticToken") | .attributes[] | select(.key=="token_id") | .value'
```

**Example Output:**
```
0x726f757465725f61707000000000000000000000000000020000000000000005
```

---

## Step 5: Enroll Remote Routers

Each warp route must know about its counterpart on the other chain. This is called "enrolling remote routers."

### 5.1 Understanding Router Enrollment

**What are Routers?**
- Each warp token contract is a "router"
- Routers must be bidirectionally enrolled to communicate
- Enrollment specifies: destination domain ID + destination router address

**Why is This Needed?**
- Security: Only enrolled routers can send/receive tokens
- Routing: Tells the contract where to send messages
- Gas: Specifies gas limit for destination chain execution

### 5.2 TIA Route Router Enrollment

#### Enroll XO Market Router on Celestia Side:

```bash
cd ../celestia-app

./build/celestia-appd tx warp enroll-remote-router \
  0x726f757465725f61707000000000000000000000000000010000000000000004 \
  1000101 \
  0x0000000000000000000000001d853F9d19c1F93B32149e99bD0c3A45E681CBc3 \
  1000000000 \
  --from relayer-wallet \
  --node http://celestia-mocha-archive-rpc.mzonder.com:26657 \
  --chain-id mocha-4 \
  --fees 1000000utia \
  --gas 300000 \
  -y
```

**Parameters:**
1. Token ID (Celestia collateral token)
2. Remote domain ID (XO Market = 1000101)
3. Remote router address (XO Market synthetic TIA contract, padded to 32 bytes)
4. Gas limit for destination execution

#### Enroll Celestia Router on XO Market Side:

```bash
cast send 0x1d853F9d19c1F93B32149e99bD0c3A45E681CBc3 \
  "enrollRemoteRouter(uint32,bytes32)" \
  1297040200 \
  0x726f757465725f61707000000000000000000000000000010000000000000004 \
  --private-key 0xYOUR_PRIVATE_KEY \
  --rpc-url https://testnet-rpc-1.xo.market
```

**Parameters:**
1. Function signature
2. Remote domain ID (Celestia = 1297040200)
3. Remote router address (Celestia collateral token ID)

### 5.3 XO Route Router Enrollment

#### Enroll XO Market Router on Celestia Side:

```bash
cd ../celestia-app

./build/celestia-appd tx warp enroll-remote-router \
  0x726f757465725f61707000000000000000000000000000020000000000000005 \
  1000101 \
  0x0000000000000000000000008Fe2521c2DAbB53c176fFBb73C3083eC319454Ac \
  1000000000 \
  --from relayer-wallet \
  --node http://celestia-mocha-archive-rpc.mzonder.com:26657 \
  --chain-id mocha-4 \
  --fees 1000000utia \
  --gas 300000 \
  -y
```

#### Enroll Celestia Router on XO Market Side:

```bash
cast send 0x8Fe2521c2DAbB53c176fFBb73C3083eC319454Ac \
  "enrollRemoteRouter(uint32,bytes32)" \
  1297040200 \
  0x726f757465725f61707000000000000000000000000000020000000000000005 \
  --private-key 0xYOUR_PRIVATE_KEY \
  --rpc-url https://testnet-rpc-1.xo.market
```

### 5.4 Verify Router Enrollment

#### Verify on Celestia:

```bash
cd ../celestia-app

./build/celestia-appd query warp remote-routers \
  0x726f757465725f61707000000000000000000000000000010000000000000004 \
  --node http://celestia-mocha-archive-rpc.mzonder.com:26657 \
  --output json | jq
```

**Expected Output:**
```json
{
  "remote_routers": [
    {
      "receiver_domain": 1000101,
      "receiver_contract": "0x0000000000000000000000001d853F9d19c1F93B32149e99bD0c3A45E681CBc3",
      "gas": "1000000000"
    }
  ]
}
```

#### Verify on EVM:

```bash
cast call 0x1d853F9d19c1F93B32149e99bD0c3A45E681CBc3 \
  "routers(uint32)(bytes32)" \
  1297040200 \
  --rpc-url https://testnet-rpc-1.xo.market
```

**Expected Output:**
```
0x726f757465725f61707000000000000000000000000000010000000000000004
```

---

## Step 6: Configure and Run Relayer

The relayer monitors both chains and automatically relays messages between them.

### 6.1 Relayer Configuration

Create `./config/relayer-config-xomarket-mocha.json`:

```json
{
  "chains": {
    "xomarkettestnet": {
      "blocks": {
        "confirmations": 1,
        "estimateBlockTime": 12,
        "reorgPeriod": 1
      },
      "chainId": 1000101,
      "displayName": "XO Market Testnet",
      "domainId": 1000101,
      "index": {
        "from": 158500,
        "chunk": 100
      },
      "isTestnet": true,
      "name": "xomarkettestnet",
      "nativeToken": {
        "decimals": 18,
        "name": "XO",
        "symbol": "XO"
      },
      "protocol": "ethereum",
      "rpcUrls": [
        {
          "http": "https://testnet-rpc-1.xo.market"
        }
      ],
      "signer": null,
      "mailbox": "0x8ED282d598296A4FCb460CBe6115446c0Dc3FD3E",
      "merkleTreeHook": "0xb58a0742AA0986eC81D28356E07612Cf23bA95b9",
      "proxyAdmin": "0x17Dc7Ff9592dCd3049B53B3558039EaCa56eFe2d",
      "validatorAnnounce": "0xa542A7DD8BAE55f35614224eCF9E287c73656F00",
      "interchainAccountRouter": "0x8218355F8F8d0057233dDc01DFD9E1Cb20b422A6",
      "interchainGasPaymaster": "0x0000000000000000000000000000000000000000"
    },
    "celestiatestnet": {
      "bech32Prefix": "celestia",
      "blocks": {
        "confirmations": 1,
        "estimateBlockTime": 12,
        "reorgPeriod": 1
      },
      "canonicalAsset": "utia",
      "chainId": "mocha-4",
      "contractAddressBytes": 32,
      "displayName": "Celestia Mocha Testnet",
      "domainId": 1297040200,
      "gasPrice": {
        "denom": "utia",
        "amount": "0.005"
      },
      "index": {
        "from": 8508130,
        "chunk": 10
      },
      "isTestnet": true,
      "name": "celestiatestnet",
      "nativeToken": {
        "decimals": 6,
        "denom": "utia",
        "name": "TIA",
        "symbol": "TIA"
      },
      "protocol": "cosmosnative",
      "restUrls": [
        {
          "http": "https://api.celestia-mocha.com"
        }
      ],
      "rpcUrls": [
        {
          "http": "http://celestia-mocha-archive-rpc.mzonder.com:26657"
        }
      ],
      "signer": {
        "type": "cosmosKey",
        "prefix": "celestia"
      },
      "slip44": 118,
      "technicalStack": "other",
      "mailbox": "0x68797065726c616e650000000000000000000000000000000000000000000003",
      "merkleTreeHook": "0x726f757465725f706f73745f6469737061746368000000030000000000000005",
      "validatorAnnounce": "0x68797065726c616e650000000000000000000000000000000000000000000003",
      "interchainGasPaymaster": "0x0000000000000000000000000000000000000000000000000000000000000000",
      "grpcUrls": [
        {
          "http": "http://public-celestia-mocha4-consensus.numia.xyz:9090"
        }
      ]
    }
  },
  "defaultRpcConsensusType": "fallback",
  "relayChains": "xomarkettestnet,celestiatestnet",
  "db": "/tmp/hyperlane-relayer-db",
  "metrics": {
    "port": 9090
  }
}
```

**Critical Configuration Points:**

1. **merkleTreeHook**: Must be correctly set for BOTH chains
   - XO Market: The deployed MerkleTreeHook from core deployment
   - Celestia: The manually created MerkleTreeHook ID

2. **index.from**: Starting block for indexing
   - Set to a block BEFORE your first warp route deployment
   - Too high = relayer won't find messages
   - Too low = relayer takes longer to sync

3. **rpcUrls**:
   - Celestia MUST use archive RPC (has historical state)
   - Regular RPCs may not have old blocks

4. **grpcUrls**: Required for Celestia (Cosmos chains use gRPC)

5. **signer**: Set to `null` for EVM, `{type: "cosmosKey", prefix: "celestia"}` for Cosmos

### 6.2 Environment Variables

Create `.env` file:

```bash
# Signer Keys
export HYP_CHAINS_XOMARKETTESTNET_SIGNER_KEY=""
export HYP_CHAINS_CELESTIATESTNET_SIGNER_KEY=""
# Default Signer
export HYP_DEFAULTSIGNER_KEY=""

# Database Path
export HYP_BASE_DB=/tmp/hyperlane-relayer-db
```

**IMPORTANT:**
- Celestia signer key must be hex format (NOT mnemonic)
- Keys must have funds on both chains for gas

### 6.3 Create Relayer Run Script

Create `./run-relayer.sh`:

```bash
#!/bin/bash

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Hyperlane Relayer: XO Market ↔ Celestia${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Configuration
RELAYER_BINARY="../hyperlane-monorepo/rust/main/target/release/relayer"
CONFIG_FILE="./config/relayer-config-xomarket-mocha.json"
DB_DIR="/tmp/hyperlane-relayer-db"

# Verify relayer binary exists
if [ ! -f "$RELAYER_BINARY" ]; then
    echo -e "${RED}ERROR: Relayer binary not found at $RELAYER_BINARY${NC}"
    echo "Please build the relayer first:"
    echo "  cd ../hyperlane-monorepo/rust/main"
    echo "  cargo build --release --bin relayer"
    exit 1
fi

# Verify config file exists
if [ ! -f "$CONFIG_FILE" ]; then
    echo -e "${RED}ERROR: Config file not found at $CONFIG_FILE${NC}"
    exit 1
fi

echo -e "${BLUE}Loading environment variables from .env...${NC}"
if [ -f .env ]; then
    source .env
else
    echo -e "${YELLOW}Warning: .env file not found. Using existing environment variables.${NC}"
fi

echo -e "${GREEN}Configuration:${NC}"
echo "  Relayer binary: $RELAYER_BINARY"
echo "  Config file: $(pwd)/$CONFIG_FILE"
echo "  Database: $DB_DIR"
echo ""

# Set environment variables for relayer
export HYP_BASE_DB="$DB_DIR"
export CONFIG_FILES="$(pwd)/$CONFIG_FILE"

echo -e "${YELLOW}⚠️  WARNING: This script uses private keys from environment variables.${NC}"
echo -e "${YELLOW}   Make sure you trust this environment and the keys have sufficient funds.${NC}"
echo ""
echo ""

echo -e "${GREEN}🚀 Starting Hyperlane Relayer...${NC}"
echo -e "${BLUE}Press Ctrl+C to stop${NC}"
echo ""

# Run the relayer
"$RELAYER_BINARY"
```

Make it executable:

```bash
chmod +x ./run-relayer.sh
```

### 6.4 Build and Run Relayer

#### Build Relayer:

```bash
cd ../hyperlane-monorepo/rust/main
cargo build --release --bin relayer
```

**Note:** If you encounter "Failed to query sequence" errors, you need to apply this fix:

In `hyperlane-base/src/contract_sync/cursors/sequence_aware/mod.rs`, change line 100-102 from:

```rust
let sequence_count = sequence_count.ok_or(ChainCommunicationError::from_other_str(
    "Failed to query sequence",
))?;
```

To:

```rust
// If sequence_count is None, it means no messages have been sent yet.
// In this case, we start from sequence 0.
let sequence_count = sequence_count.unwrap_or(0);
```

Then rebuild.

#### Run Relayer:

```bash
cd /path/to/hyperlane-ops
./run-relayer.sh
```

**Successful startup looks like:**

```
Agent relayer starting up with version 79998e9d66e6d27ca13302a0d07e9ff8b442b7c2
Loading settings: 2025-10-21T14:26:29.849824Z
INFO hyperlane_base::db::rocks: Opening existing db, path: /private/tmp/hyperlane-relayer-db
INFO relayer::relayer: Whitelist configuration
INFO relayer::relayer: Starting tokio console server
```

### 6.5 Relayer Database Management

**When to Clear Database:**

The relayer maintains a local database of indexed messages. You may need to clear it if:
- Switching between different warp routes
- Seeing sequence mismatch warnings
- Relayer shows stale data

**Clear Database:**

```bash
pkill -9 -f relayer
rm -rf /tmp/hyperlane-relayer-db
```

**Then restart the relayer** - it will re-index from the block specified in `index.from`.

---

## Step 7: Testing Transfers

### 7.1 Transfer TIA from Celestia to XO Market

#### Step 1: Check Initial Balances

```bash
# Celestia TIA balance
cd ../celestia-app
./build/celestia-appd query bank balances celestia1lg0e9n4pt29lpq2k4ptue4ckw09dx0aujlpe4j \
  --node http://celestia-mocha-archive-rpc.mzonder.com:26657 \
  --output json | jq -r '.balances[] | select(.denom=="utia")'

# XO Market synthetic TIA balance
cast call 0x1d853F9d19c1F93B32149e99bD0c3A45E681CBc3 \
  "balanceOf(address)(uint256)" \
  0xc259e540167B7487A89b45343F4044d5951cf871 \
  --rpc-url https://testnet-rpc-1.xo.market
```

#### Step 2: Convert Destination Address

For Cosmos → EVM transfers, you need to convert the EVM address to hex format:

```bash
# EVM address: 0xc259e540167B7487A89b45343F4044d5951cf871
# Padded to 32 bytes: 0x000000000000000000000000c259e540167B7487A89b45343F4044d5951cf871
```

#### Step 3: Initiate Transfer

```bash
cd ../celestia-app

./build/celestia-appd tx warp transfer \
  0x726f757465725f61707000000000000000000000000000010000000000000004 \
  1000101 \
  0x000000000000000000000000c259e540167B7487A89b45343F4044d5951cf871 \
  1000000utia \
  --from relayer-wallet \
  --node http://celestia-mocha-archive-rpc.mzonder.com:26657 \
  --chain-id mocha-4 \
  --fees 1000000utia \
  --gas 300000 \
  -y
```

**Parameters:**
1. Token ID (Celestia collateral token)
2. Destination domain (XO Market = 1000101)
3. Recipient address (32-byte padded EVM address)
4. Amount (1000000 utia = 1 TIA, 6 decimals)

#### Step 4: Monitor Relayer

Watch relayer logs for:

```
INFO relayer::msg::processor: Found message to process
INFO lander::dispatcher: Dispatching transaction
INFO lander::dispatcher: Transaction confirmed
```

#### Step 5: Verify Receipt

```bash
# Check XO Market synthetic TIA balance (should increase)
cast call 0x1d853F9d19c1F93B32149e99bD0c3A45E681CBc3 \
  "balanceOf(address)(uint256)" \
  0xc259e540167B7487A89b45343F4044d5951cf871 \
  --rpc-url https://testnet-rpc-1.xo.market

# Convert wei to TIA (6 decimals)
# Expected: 1000000 (1 TIA)
```

### 7.2 Transfer XO from XO Market to Celestia

#### Step 1: Convert Destination Address

For EVM → Cosmos transfers, convert bech32 to hex:

```bash
cd ../celestia-app

./build/celestia-appd debug addr celestia1lg0e9n4pt29lpq2k4ptue4ckw09dx0aujlpe4j
# Output: FA1F92CEA15A8BF08156A857CCD71673CAD33FBC

# Padded: 0x000000000000000000000000FA1F92CEA15A8BF08156A857CCD71673CAD33FBC
```

#### Step 2: Initiate Transfer

```bash
cast send 0x8Fe2521c2DAbB53c176fFBb73C3083eC319454Ac \
  "transferRemote(uint32,bytes32,uint256)" \
  1297040200 \
  0x000000000000000000000000FA1F92CEA15A8BF08156A857CCD71673CAD33FBC \
  10000000000000000 \
  --value 0.01ether \
  --private-key 0xYOUR_PRIVATE_KEY \
  --rpc-url https://testnet-rpc-1.xo.market
```

**Parameters:**
1. Function signature
2. Destination domain (Celestia = 1297040200)
3. Recipient address (32-byte padded, hex format of Celestia address)
4. Amount (10000000000000000 = 0.01 XO, 18 decimals)
5. `--value`: For native token transfers, must send the amount

#### Step 3: Verify Receipt

```bash
cd ../celestia-app

./build/celestia-appd query bank balances celestia1lg0e9n4pt29lpq2k4ptue4ckw09dx0aujlpe4j \
  --node http://celestia-mocha-archive-rpc.mzonder.com:26657 \
  --output json | jq -r '.balances[] | select(.denom | contains("726f757465725f61707000000000000000000000000000020000000000000005"))'
```

**Expected Output:**
```json
{
  "denom": "hyperlane/0x726f757465725f61707000000000000000000000000000020000000000000005",
  "amount": "10000000000000000"
}
```

---

## Step 8: How to Check It Works

### 8.1 Pre-Transfer Verification

Before sending any transfers, verify the setup:

#### Check Router Enrollment:

```bash
# Celestia side
cd ../celestia-app
./build/celestia-appd query warp remote-routers <TOKEN_ID> \
  --node http://celestia-mocha-archive-rpc.mzonder.com:26657 \
  --output json

# EVM side
cast call <WARP_TOKEN_ADDRESS> "routers(uint32)(bytes32)" <REMOTE_DOMAIN> \
  --rpc-url <RPC_URL>
```

Should return non-zero values.

#### Check MerkleTreeHook:

```bash
# EVM
cast call <MERKLE_TREE_HOOK_ADDRESS> "count()(uint32)" --rpc-url <RPC_URL>

# Celestia
cd ../celestia-app
./build/celestia-appd query hyperlane hooks merkle-tree-hook <HOOK_ID> \
  --node http://celestia-mocha-archive-rpc.mzonder.com:26657 \
  --output json
```

Should return the current message count.

#### Check Relayer Status:

```bash
# Relayer should be running
ps aux | grep relayer

# Check recent logs
tail -f relayer.log
```

Should show syncing activity, no errors.

### 8.2 Post-Transfer Verification

#### 1. Check Transaction Success:

**Celestia:**
```bash
cd ../celestia-app
./build/celestia-appd query tx <TX_HASH> \
  --node http://celestia-mocha-archive-rpc.mzonder.com:26657 \
  --output json | jq -r '.code'
```

Should return `0` (success).

**EVM:**
```bash
cast receipt <TX_HASH> --rpc-url <RPC_URL>
```

Look for `status: 1 (success)`.

#### 2. Check Message Dispatch Event:

**EVM:**
```bash
cast receipt <TX_HASH> --rpc-url <RPC_URL> | grep -A 5 "Dispatch"
```

Should show `Dispatch` event with message ID.

**Celestia:**
```bash
cd ../celestia-app
./build/celestia-appd query tx <TX_HASH> \
  --node http://celestia-mocha-archive-rpc.mzonder.com:26657 \
  --output json | jq -r '.events[] | select(.type=="hyperlane.core.v1.EventDispatch")'
```

#### 3. Monitor Relayer Processing:

Watch relayer logs for:

```
INFO relayer::msg::processor: Found message to process,
  nonce: X,
  origin: <source_chain>,
  destination: <dest_chain>

INFO lander::dispatcher: Dispatching transaction

INFO lander::dispatcher: Transaction confirmed,
  txid: <tx_hash>
```

#### 4. Verify Balance Changes:

**Source Chain (tokens locked/burned):**
```bash
# Before and after should show decrease
```

**Destination Chain (tokens minted/released):**
```bash
# Before and after should show increase
```

#### 5. Check MerkleTreeHook Count Increased:

```bash
# Should increment by 1 after each message
cast call <MERKLE_TREE_HOOK> "count()(uint32)" --rpc-url <RPC_URL>
```

---

## Example Transactions

### Example 1: TIA Transfer (Celestia → XO Market)

**Transaction Hash:** `D3DBCEC059E0E99A0FCF802D8F4821372A43E0831F939D81408E045EC300E6EC`

**Details:**
- **Source**: Celestia Mocha-4, Block 8508131
- **Amount**: 1 TIA (1000000 utia)
- **Sender**: celestia1lg0e9n4pt29lpq2k4ptue4ckw09dx0aujlpe4j
- **Recipient**: 0xc259e540167B7487A89b45343F4044d5951cf871 (XO Market)

**Verification:**

```bash
# Check Celestia transaction
cd ../celestia-app
./build/celestia-appd query tx D3DBCEC059E0E99A0FCF802D8F4821372A43E0831F939D81408E045EC300E6EC \
  --node http://celestia-mocha-archive-rpc.mzonder.com:26657 \
  --output json | jq

# Verify XO Market received synthetic TIA
cast call 0x1d853F9d19c1F93B32149e99bD0c3A45E681CBc3 \
  "balanceOf(address)(uint256)" \
  0xc259e540167B7487A89b45343F4044d5951cf871 \
  --rpc-url https://testnet-rpc-1.xo.market
# Result: 1000000 (1 TIA with 6 decimals)
```

**Relayer Log Snippet:**
```
INFO relayer::msg::processor: Found message to process, nonce: 14, origin: celestiatestnet
INFO lander::dispatcher: Dispatching transaction to xomarkettestnet
INFO lander::dispatcher: Transaction confirmed, txid: 0x...
```

### Example 2: TIA Return Transfer (XO Market → Celestia)

**Transaction Hash:** `0xcc99afe3bc5c98c3ea5195cf86b908500975d0542d27742c7172f02ccada9f4c`

**Details:**
- **Source**: XO Market, Block 159148
- **Amount**: 0.5 TIA (500000 with 6 decimals)
- **Sender**: 0xc259e540167B7487A89b45343F4044d5951cf871
- **Recipient**: celestia1lg0e9n4pt29lpq2k4ptue4ckw09dx0aujlpe4j

**Verification:**

```bash
# Check XO Market transaction
cast receipt 0xcc99afe3bc5c98c3ea5195cf86b908500975d0542d27742c7172f02ccada9f4c \
  --rpc-url https://testnet-rpc-1.xo.market

# Verify Celestia received native TIA back
cd ../celestia-app
./build/celestia-appd query bank balances celestia1lg0e9n4pt29lpq2k4ptue4ckw09dx0aujlpe4j \
  --node http://celestia-mocha-archive-rpc.mzonder.com:26657 \
  --output json | jq -r '.balances[] | select(.denom=="utia")'
# Balance should increase by 500000 utia
```

### Example 3: XO Transfer (XO Market → Celestia)

**Transaction Hash:** `0xd76447420f794e7d6f672f097713eac7d480f2d5d8c0c6b3a39464f0f014c4c8`

**Details:**
- **Source**: XO Market, Block 159399
- **Amount**: 0.01 XO (10000000000000000 wei, 18 decimals)
- **Sender**: 0xc259e540167B7487A89b45343F4044d5951cf871
- **Recipient**: celestia1lg0e9n4pt29lpq2k4ptue4ckw09dx0aujlpe4j

**Verification:**

```bash
# Check XO Market transaction
cast receipt 0xd76447420f794e7d6f672f097713eac7d480f2d5d8c0c6b3a39464f0f014c4c8 \
  --rpc-url https://testnet-rpc-1.xo.market

# Verify Celestia received synthetic XO
cd ../celestia-app
./build/celestia-appd query bank balances celestia1lg0e9n4pt29lpq2k4ptue4ckw09dx0aujlpe4j \
  --node http://celestia-mocha-archive-rpc.mzonder.com:26657 \
  --output json | jq -r '.balances[] | select(.denom | contains("726f757465725f61707000000000000000000000000000020000000000000005"))'
# Result: {"denom": "hyperlane/0x726f757465725f61707000000000000000000000000000020000000000000005", "amount": "10000000000000000"}
```

**Receipt Details:**
```json
{
  "blockNumber": 159399,
  "status": "1 (success)",
  "gasUsed": 120458,
  "logs": [
    {
      "topics": ["0x769f711d20c679153d382254f59892613b58a97cc876b249134ac25c80f9c814"],
      "data": "0x... (Dispatch event)"
    }
  ]
}
```

---

## Troubleshooting

### Issue 1: "Failed to query sequence" Error

**Symptoms:**
```
WARN hyperlane_core::rpc_clients::retry: Retrying call, error: EyreError(Failed to query sequence)
```

**Cause:** Missing or incorrectly configured MerkleTreeHook.

**Solution:**
1. Verify MerkleTreeHook exists:
   ```bash
   cast call <MERKLE_TREE_HOOK> "count()(uint32)" --rpc-url <RPC_URL>
   ```
2. Check relayer config has correct `merkleTreeHook` address
3. For Celestia, ensure you created MerkleTreeHook manually (NoopISM deployment doesn't include it)
4. Apply the relayer code fix (unwrap_or(0) instead of ok_or)

### Issue 2: Sequence Count Mismatch

**Symptoms:**
```
WARN Current sequence is greater than the onchain sequence count,
  current_sequence: 2, onchain_sequence_count: 1
```

**Cause:** Relayer database has stale data from previous deployments.

**Solution:**
```bash
pkill -9 -f relayer
rm -rf /tmp/hyperlane-relayer-db
./run-relayer.sh
```

### Issue 3: No Messages Being Relayed

**Possible Causes:**

1. **Routers not enrolled**: Verify with queries in section 5.4
2. **Relayer not synced**: Check `index.from` is before first message
3. **Wrong signer keys**: Check environment variables
4. **Insufficient gas**: Fund relayer addresses on both chains

**Debug Steps:**

```bash
# 1. Check relayer is running
ps aux | grep relayer

# 2. Check relayer logs
tail -f relayer.log

# 3. Verify message was dispatched
cast receipt <TX_HASH> --rpc-url <RPC_URL> | grep Dispatch

# 4. Check MerkleTreeHook count increased
cast call <MERKLE_TREE_HOOK> "count()(uint32)" --rpc-url <RPC_URL>

# 5. Verify relayer indexed the message
# Look for "Found message to process" in logs
```

### Issue 4: Invalid Signer Key Format

**Symptoms:**
```
Expected a valid private key in hex, base58 or bech32
Caused by: Invalid length of base58 string
```

**Cause:** Using mnemonic instead of hex private key for Celestia.

**Solution:**
- Celestia signer must be hex format: `0x6e30efb1d3ebd30d1ba08c8d5fc9b190e08394009dc1dd787a69e60c33288a8c`
- NOT mnemonic: `"word word word..."`
- Set in environment: `HYP_CHAINS_CELESTIATESTNET_SIGNER_KEY=0x...`

### Issue 5: RPC Node Errors

**Symptoms:**
```
height 4 is not available, lowest height is 8299501
```

**Cause:** Using non-archive RPC that doesn't have historical blocks.

**Solution:**
- Use archive RPC for Celestia: `http://celestia-mocha-archive-rpc.mzonder.com:26657`
- Adjust `index.from` to a more recent block
- Remove problematic RPCs from fallback list

### Issue 6: Address Format Errors

**Cosmos → EVM:**
```bash
# Convert bech32 to hex
cd ../celestia-app
./build/celestia-appd debug addr celestia1lg0e9n4pt29lpq2k4ptue4ckw09dx0aujlpe4j
# Use hex output, pad to 32 bytes: 0x000000000000000000000000FA1F92CEA15A8BF08156A857CCD71673CAD33FBC
```

**EVM → Cosmos:**
```bash
# Pad EVM address to 32 bytes
# 0xc259e540167B7487A89b45343F4044d5951cf871
# becomes: 0x000000000000000000000000c259e540167B7487A89b45343F4044d5951cf871
```

---

## Summary

This guide covered:

1. ✅ **Chain Registry Setup**: Metadata for both chains
2. ✅ **Core Deployment**: Mailbox + MerkleTreeHook on both chains
3. ✅ **MerkleTreeHook Importance**: Why it's critical and how to create it
4. ✅ **Warp Routes**: Deploying native/synthetic token pairs
5. ✅ **Router Enrollment**: Bidirectional enrollment for communication
6. ✅ **Relayer Configuration**: Complete setup and troubleshooting
7. ✅ **Testing Transfers**: Step-by-step transfer instructions
8. ✅ **Verification**: How to confirm everything works
9. ✅ **Real Examples**: Actual transaction hashes and results

**Key Takeaways:**

- **MerkleTreeHook is mandatory** - relayer will fail without it
- **Router enrollment must be bidirectional** - both sides need to know about each other
- **Address format matters** - always pad to 32 bytes for cross-chain transfers
- **Archive RPCs are required** - for historical message indexing
- **Clear database when switching routes** - prevents sequence mismatches

---

## Deployed Addresses Reference

### XO Market Testnet

**Core:**
- Mailbox: `0x8ED282d598296A4FCb460CBe6115446c0Dc3FD3E`
- MerkleTreeHook: `0xb58a0742AA0986eC81D28356E07612Cf23bA95b9`
- ProxyAdmin: `0x17Dc7Ff9592dCd3049B53B3558039EaCa56eFe2d`
- ValidatorAnnounce: `0xa542A7DD8BAE55f35614224eCF9E287c73656F00`

**Warp Routes:**
- Synthetic TIA: `0x1d853F9d19c1F93B32149e99bD0c3A45E681CBc3`
- Native XO: `0x8Fe2521c2DAbB53c176fFBb73C3083eC319454Ac`

### Celestia Mocha-4 Testnet

**Core:**
- Mailbox: `0x68797065726c616e650000000000000000000000000000000000000000000003`
- MerkleTreeHook: `0x726f757465725f706f73745f6469737061746368000000030000000000000005`

**Warp Routes:**
- Collateral TIA: `0x726f757465725f61707000000000000000000000000000010000000000000004`
- Synthetic XO: `0x726f757465725f61707000000000000000000000000000020000000000000005`

---

**Documentation Version:** 1.0
**Last Updated:** October 21, 2025
**Chains:** Celestia Mocha-4 ↔ XO Market Testnet
