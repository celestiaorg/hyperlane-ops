#!/bin/bash

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Hyperlane Relayer: Eden ↔ Celestia${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Configuration
RELAYER_BINARY="../hyperlane-monorepo/rust/main/target/release/relayer"
CONFIG_FILE="./config/relayer-config-eden-mocha.json"
DB_DIR="/tmp/hyperlane-relayer-db-eden"

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
    set -a
    source .env
    set +a
else
    echo -e "${YELLOW}Warning: .env file not found. Using existing environment variables.${NC}"
fi

# Check if required keys are set
if [ -z "${HYP_CHAINS_EDENTESTNET_SIGNER_KEY:-}" ]; then
    echo -e "${RED}ERROR: HYP_CHAINS_EDENTESTNET_SIGNER_KEY is not set${NC}"
    echo "Please set it in your .env file"
    exit 1
fi

if [ -z "${HYP_CHAINS_CELESTIATESTNET_SIGNER_KEY:-}" ]; then
    echo -e "${RED}ERROR: HYP_CHAINS_CELESTIATESTNET_SIGNER_KEY is not set${NC}"
    echo "Please set it in your .env file"
    exit 1
fi

echo -e "${GREEN}Configuration:${NC}"
echo "  Relayer binary: $RELAYER_BINARY"
echo "  Config file: $(pwd)/$CONFIG_FILE"
echo "  Database: $DB_DIR"
echo "  Eden Domain: 2147483647"
echo "  Celestia Domain: 1297040200"
echo ""

# Set environment variables for relayer
export HYP_BASE_DB="$DB_DIR"
export CONFIG_FILES="$(pwd)/$CONFIG_FILE"

echo -e "${BLUE}Chain Configuration:${NC}"
echo -e "${GREEN}Eden Testnet:${NC}"
echo -e "  RPC: https://ev-reth-eden-testnet.binarybuilders.services:8545"
echo -e "  Mailbox: 0xBdEfA74aCf073Fc5c8961d76d5DdA87B1Be2C1b0"
echo ""
echo -e "${GREEN}Celestia Mocha-4:${NC}"
echo -e "  RPC: http://celestia-mocha-archive-rpc.mzonder.com:26657"
echo -e "  Mailbox: 0x68797065726c616e650000000000000000000000000000000000000000000003"
echo ""

echo -e "${YELLOW}⚠️  WARNING: This script uses private keys from environment variables.${NC}"
echo -e "${YELLOW}   Make sure you trust this environment and the keys have sufficient funds.${NC}"
echo ""

# Ask for confirmation
read -p "Start the relayer? (y/n) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo -e "${YELLOW}Relayer startup cancelled.${NC}"
    exit 0
fi

echo ""
echo -e "${GREEN}🚀 Starting Hyperlane Relayer...${NC}"
echo -e "${BLUE}Press Ctrl+C to stop${NC}"
echo ""

# Run the relayer
"$RELAYER_BINARY"
