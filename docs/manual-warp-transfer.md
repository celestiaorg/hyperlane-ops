# Manual Warp Transfer (USDC Sepolia <-> Celestia <-> Eden)

This guide covers manual Warp transfers for:

- `USDC/celestiatestnet-edentestnet-sepolia`

It includes:
- EVM-origin transfers (using Foundry `cast`)
- Celestia-origin transfers (using `celestia-appd`)

## Route Constants

```bash
# Domains
SEPOLIA_DOMAIN=11155111
CELESTIA_DOMAIN=1297040200
EDEN_DOMAIN=2147483647

# Routers and token addresses
SEPOLIA_USDC_ROUTER=0x22cCd0e1efc2beF46143eA00e3868A35ebA16113
EDEN_USDC_ROUTER=0x0C1c5a78669ea6cb269883ad1B65334319Aacfd7
SEPOLIA_USDC=0xf77764d1E232Ec088150a3E434678768f8774f21
CELESTIA_USDC_TOKEN_ID=0x726f757465725f61707000000000000000000000000000020000000000000024

# RPC endpoints
SEPOLIA_RPC=https://gateway.tenderly.co/public/sepolia
EDEN_RPC=https://eden-rpc-proxy-production.up.railway.app/rpc
CELESTIA_RPC=https://rpc-1.testnet.celestia.nodes.guru
```

## Prerequisites

- Sender has native gas token on the origin chain.
- Sender has token balance on the origin chain.
- Relayer is running (or you relay through your normal ops flow).

For EVM-origin transactions:

```bash
export PRIVATE_KEY=0x...
```

For Celestia-origin transactions:

```bash
# Example: recover/import key first, then use --from owner in tx commands
echo "$HYP_MNEMONIC" | celestia-appd keys add owner --recover
```

## Recipient Formatting

Warp recipients are `bytes32`.

For an EVM recipient `0xabc...` use left-padded 32-byte hex:

```bash
EVM_RECIPIENT=0xc259e540167B7487A89b45343F4044d5951cf871
RECIPIENT_B32=0x000000000000000000000000${EVM_RECIPIENT#0x}
```

## 1. Preflight Checks

### EVM routers

```bash
cast call --rpc-url $SEPOLIA_RPC $SEPOLIA_USDC_ROUTER "routers(uint32)(bytes32)" $EDEN_DOMAIN
cast call --rpc-url $EDEN_RPC $EDEN_USDC_ROUTER "routers(uint32)(bytes32)" $SEPOLIA_DOMAIN
```

### Celestia token and routers

```bash
celestia-appd query warp token $CELESTIA_USDC_TOKEN_ID --node $CELESTIA_RPC -o json
celestia-appd query warp remote-routers $CELESTIA_USDC_TOKEN_ID --node $CELESTIA_RPC -o json
```

### Gas quote checks

```bash
# EVM
cast call --rpc-url $SEPOLIA_RPC $SEPOLIA_USDC_ROUTER "quoteGasPayment(uint32)(uint256)" $EDEN_DOMAIN
cast call --rpc-url $EDEN_RPC $EDEN_USDC_ROUTER "quoteGasPayment(uint32)(uint256)" $SEPOLIA_DOMAIN

# Celestia
celestia-appd query warp quote-transfer $CELESTIA_USDC_TOKEN_ID $SEPOLIA_DOMAIN --node $CELESTIA_RPC -o json
```

## 2. EVM Origin Transfers

### Sepolia -> Eden (collateral -> synthetic)

```bash
DEST_RECIPIENT_EVM=0xYourEdenRecipient
AMOUNT_UNITS=1000000 # 1 USDC (6 decimals)
RECIPIENT_B32=$(cast abi-encode "f(address)" "$DEST_RECIPIENT_EVM")
GAS_QUOTE=$(cast call --rpc-url $SEPOLIA_RPC $SEPOLIA_USDC_ROUTER "quoteGasPayment(uint32)(uint256)" $EDEN_DOMAIN)

# Approve collateral spend
cast send --rpc-url $SEPOLIA_RPC --private-key $PRIVATE_KEY \
  $SEPOLIA_USDC "approve(address,uint256)" $SEPOLIA_USDC_ROUTER $AMOUNT_UNITS

# Transfer
cast send --rpc-url $SEPOLIA_RPC --private-key $PRIVATE_KEY \
  --value $GAS_QUOTE \
  $SEPOLIA_USDC_ROUTER \
  "transferRemote(uint32,bytes32,uint256)" \
  $EDEN_DOMAIN $RECIPIENT_B32 $AMOUNT_UNITS
```

### Eden -> Sepolia (synthetic -> collateral)

```bash
DEST_RECIPIENT_EVM=0xYourSepoliaRecipient
AMOUNT_UNITS=1000000 # 1 USDC (6 decimals)
RECIPIENT_B32=$(cast abi-encode "f(address)" "$DEST_RECIPIENT_EVM")
GAS_QUOTE=$(cast call --rpc-url $EDEN_RPC $EDEN_USDC_ROUTER "quoteGasPayment(uint32)(uint256)" $SEPOLIA_DOMAIN)

cast send --rpc-url $EDEN_RPC --private-key $PRIVATE_KEY \
  --value $GAS_QUOTE \
  $EDEN_USDC_ROUTER \
  "transferRemote(uint32,bytes32,uint256)" \
  $SEPOLIA_DOMAIN $RECIPIENT_B32 $AMOUNT_UNITS
```

## 3. Celestia Origin Transfers (`celestia-appd`)

### Dry run / unsigned generation (recommended first)

```bash
RECIPIENT_B32=0x000000000000000000000000c259e540167b7487a89b45343f4044d5951cf871
AMOUNT_UNITS=1000000 # 1 USDC (6 decimals)

celestia-appd tx warp transfer \
  $CELESTIA_USDC_TOKEN_ID \
  $SEPOLIA_DOMAIN \
  $RECIPIENT_B32 \
  $AMOUNT_UNITS \
  --from celestia1lg0e9n4pt29lpq2k4ptue4ckw09dx0aujlpe4j \
  --chain-id mocha-4 \
  --node $CELESTIA_RPC \
  --fees 800utia \
  --max-hyperlane-fee 0utia \
  --generate-only \
  -o json
```

### Broadcast transfer

```bash
celestia-appd tx warp transfer \
  $CELESTIA_USDC_TOKEN_ID \
  $SEPOLIA_DOMAIN \
  $RECIPIENT_B32 \
  $AMOUNT_UNITS \
  --from owner \
  --chain-id mocha-4 \
  --node $CELESTIA_RPC \
  --fees 800utia \
  --max-hyperlane-fee 0utia \
  --yes
```

For Celestia -> Eden, change destination domain to `$EDEN_DOMAIN`.

## 4. Verify Balances

```bash
# Sepolia collateral token balance
cast call --rpc-url $SEPOLIA_RPC $SEPOLIA_USDC "balanceOf(address)(uint256)" 0xYourAddress

# Eden synthetic token balance (router is also token contract)
cast call --rpc-url $EDEN_RPC $EDEN_USDC_ROUTER "balanceOf(address)(uint256)" 0xYourAddress
```

On Celestia, inspect token/bridged supply via:

```bash
celestia-appd query warp token $CELESTIA_USDC_TOKEN_ID --node $CELESTIA_RPC -o json
celestia-appd query warp bridged-supply $CELESTIA_USDC_TOKEN_ID --node $CELESTIA_RPC -o json
```

## Troubleshooting

- `No router enrolled for domain`: destination domain is not enrolled on origin router.
- `ERC20: insufficient allowance`: Sepolia approval missing/too low.
- `invalid decimal coin expression: 0`: pass `--max-hyperlane-fee` with denom, e.g. `0utia`.
- Dispatched but not delivered: relayer down or destination gas settings insufficient.
