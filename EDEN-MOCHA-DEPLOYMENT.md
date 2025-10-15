# Eden-Mocha Warp Route Deployment Summary

## Deployed Components

### Eden Testnet (EVM - Domain 2147483647)
- **Synthetic TIA Token**: `0x75C038819c6eB5A51248e4ea7B6431d79f85Bd46`
- **Mailbox**: `0xBdEfA74aCf073Fc5c8961d76d5DdA87B1Be2C1b0`
- **Remote Router Enrolled**: ✅ Points to Mocha token `0x726f757465725f61707000000000000000000000000000010000000000000003`
- **Destination Gas**: 300,000

### Celestia Mocha Testnet (Cosmos - Domain 1297040200)
- **Collateral Token ID**: `0x726f757465725f61707000000000000000000000000000010000000000000003`
- **Collateral Asset**: `utia` (native TIA)
- **Mailbox**: `0x68797065726c616e650000000000000000000000000000000000000000000000`
- **Owner**: `celestia1lg0e9n4pt29lpq2k4ptue4ckw09dx0aujlpe4j`
- **Remote Router Enrolled**: ✅ Points to Eden token `0x00000000000000000000000075C038819c6eB5A51248e4ea7B6431d79f85Bd46`

## Configuration Files

### Warp Route Config
- **Location**: `~/.hyperlane/deployments/warp_routes/TIA/eden-mocha-warp-eden-only-config.yaml`
- **Repo Config**: `configs/eden-mocha-warp-config.yaml`

### Chain Metadata
- **Eden**: `~/.hyperlane/chains/edentestnet/metadata.yaml`
- **Mocha**: `~/.hyperlane/chains/celestiatestnet/metadata.yaml`
  - **Correct Domain ID**: 1297040200 (updated from incorrect 69420)
  - **RPC**: `http://public-celestia-mocha4-consensus.numia.xyz:26657`
  - **gRPC**: `public-celestia-mocha4-consensus.numia.xyz:9090`
  - **REST**: `https://api-mocha.pops.one`

## Testing the Warp Route

### Send TIA from Mocha to Eden

```bash
cd ~/projects/celestia/celestia-app

./build/celestia-appd tx warp transfer \
  0x726f757465725f61707000000000000000000000000000010000000000000003 \
  2147483647 \
  0x000000000000000000000000c259e540167B7487A89b45343F4044d5951cf871 \
  1000000 \
  --chain-id mocha-4 \
  --from relayer-wallet \
  --max-hyperlane-fee 50000utia \
  --node http://public-celestia-mocha4-consensus.numia.xyz:26657 \
  --gas auto \
  --gas-adjustment 1.5 \
  --gas-prices 0.005utia
```

This will:
1. Lock 1 TIA (1,000,000 utia) on Mocha
2. Dispatch a Hyperlane message to Eden
3. **Requires a relayer to process the message**
4. Mint 1 synthetic TIA on Eden to the recipient

### Send TIA from Eden to Mocha

```bash
# Using Hyperlane CLI or cast
hyperlane warp send \
  --warp ~/.hyperlane/deployments/warp_routes/TIA/eden-mocha-warp-eden-only-config.yaml \
  --origin edentestnet \
  --destination celestiatestnet \
  --amount 1000000 \
  --recipient celestia1lg0e9n4pt29lpq2k4ptue4ckw09dx0aujlpe4j \
  --registry ~/.hyperlane -y
```

## Relayer Setup (TODO)

The Hyperlane CLI relayer has limitations with Cosmos chains. For production use, deploy the standalone Hyperlane Rust agent:

1. **Clone Hyperlane monorepo**: https://github.com/hyperlane-xyz/hyperlane-monorepo
2. **Build the relayer**: `cargo build --release -p relayer`
3. **Configure** with both Eden and Mocha metadata
4. **Run** the relayer to process cross-chain messages

### Alternative: Manual Message Relay

For testing without a full relayer, you can manually relay messages using the Hyperlane CLI:

```bash
# After sending a message, get the message ID from the transaction
# Then use hyperlane status to check and manually relay if needed
hyperlane status --id <MESSAGE_ID> --registry ~/.hyperlane
```

## Important Notes

- ✅ Remote routers are enrolled on both chains
- ✅ Domain IDs are correct (Eden: 2147483647, Mocha: 1297040200)
- ✅ Warp route contracts deployed
- ⚠️ Relayer needed to process cross-chain messages
- 📝 Save private keys for both Eden and Mocha signers

## Tools Used

- **hyp tool**: Built custom CLI for Cosmos native operations (`/Users/blasrodriguezgarciairizar/projects/celestia/celestia-zkevm-hl-testnet/hyperlane/hyp`)
- **celestia-appd**: For Mocha transactions
- **hyperlane CLI**: For Eden EVM operations

## Transaction Examples

### Successful Remote Router Enrollment
- **TX Hash**: `A41ABB21DFA421F3AE212AC264A2ACA4627E7DAA3DA41BFF54197C0748C0F2CE`
- **Event**: `hyperlane.warp.v1.EventEnrollRemoteRouter`
- **Verified**: Remote router shows in `celestia-appd query warp remote-routers` output

## Next Steps

1. Set up a production relayer (Rust agent recommended)
2. Test end-to-end message flow
3. Monitor relayer performance and gas costs
4. Document any issues or optimizations needed
