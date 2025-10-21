#!/bin/bash

# Hyperlane Relayer Script for XO Market <-> Celestia Mocha-4
# This script runs the Hyperlane relayer to facilitate message passing between the chains

set -euo pipefail

# Color codes for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RELAYER_BINARY="../hyperlane-monorepo/rust/main/target/release/relayer"
CONFIG_DIR="${SCRIPT_DIR}/config"
CONFIG_FILE="${CONFIG_DIR}/relayer-config-xomarket-mocha.json"
DB_DIR="/tmp/hyperlane-relayer-db"

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Hyperlane Relayer: XO Market ↔ Celestia${NC}"
echo -e "${BLUE}========================================${NC}"
echo

# Check if relayer binary exists
if [ ! -f "$RELAYER_BINARY" ]; then
    echo -e "${RED}Error: Relayer binary not found at $RELAYER_BINARY${NC}"
    echo -e "${YELLOW}Building relayer...${NC}"
    cd ../hyperlane-monorepo/rust/main
    cargo build --release --bin relayer
    cd "$SCRIPT_DIR"
    echo -e "${GREEN}✓ Relayer built successfully${NC}"
fi

# Create config directory if it doesn't exist
mkdir -p "$CONFIG_DIR"

# Copy config file to config directory if it doesn't exist there
if [ ! -f "$CONFIG_FILE" ]; then
    if [ -f "${SCRIPT_DIR}/relayer-config-xomarket-mocha.json" ]; then
        cp "${SCRIPT_DIR}/relayer-config-xomarket-mocha.json" "$CONFIG_DIR/"
    else
        echo -e "${RED}Error: Config file not found${NC}"
        exit 1
    fi
fi

# Load environment variables from .env if it exists
if [ -f "${SCRIPT_DIR}/.env" ]; then
    echo -e "${BLUE}Loading environment variables from .env...${NC}"
    set -a
    source "${SCRIPT_DIR}/.env"
    set +a
fi

# Set required environment variables
# Check if keys are set
if [ -z "${HYP_KEY:-}" ]; then
    echo -e "${RED}Error: HYP_KEY environment variable is not set${NC}"
    echo -e "${YELLOW}Please set HYP_KEY in your .env file or export it before running this script${NC}"
    exit 1
fi

if [ -z "${CELESTIA_SIGNER_KEY:-}" ]; then
    echo -e "${RED}Error: CELESTIA_SIGNER_KEY environment variable is not set${NC}"
    echo -e "${YELLOW}Please set CELESTIA_SIGNER_KEY in your .env file or export it before running this script${NC}"
    exit 1
fi

export XO_SIGNER_KEY="${HYP_KEY}"
# For Celestia, cosmosKey type also expects hex private key (not mnemonic)
export CELESTIA_SIGNER_KEY="${CELESTIA_SIGNER_KEY}"

# Create DB directory if it doesn't exist
mkdir -p "$DB_DIR"

# Display configuration
echo -e "${GREEN}Configuration:${NC}"
echo -e "  Relayer binary: ${RELAYER_BINARY}"
echo -e "  Config file: ${CONFIG_FILE}"
echo -e "  Database: ${DB_DIR}"
echo -e "  XO Market Domain: 1000101"
echo -e "  Celestia Domain: 1297040200"
echo

# Display chain info
echo -e "${BLUE}Chain Configuration:${NC}"
echo -e "${GREEN}XO Market Testnet:${NC}"
echo -e "  RPC: https://testnet-rpc-1.xo.market"
echo -e "  Mailbox: 0x8ED282d598296A4FCb460CBe6115446c0Dc3FD3E"
echo -e "  Warp Token: 0x1d853F9d19c1F93B32149e99bD0c3A45E681CBc3"
echo
echo -e "${GREEN}Celestia Mocha-4:${NC}"
echo -e "  RPC: http://public-celestia-mocha4-consensus.numia.xyz:26657"
echo -e "  gRPC: public-celestia-mocha4-consensus.numia.xyz:9090"
echo -e "  Mailbox: 0x68797065726c616e650000000000000000000000000000000000000000000003"
echo

# Warning about keys
echo -e "${YELLOW}⚠️  WARNING: This script uses private keys from environment variables.${NC}"
echo -e "${YELLOW}   Make sure you trust this environment and the keys have sufficient funds.${NC}"
echo

# Ask for confirmation
read -p "Start the relayer? (y/n) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo -e "${YELLOW}Relayer startup cancelled.${NC}"
    exit 0
fi

echo
echo -e "${GREEN}🚀 Starting Hyperlane Relayer...${NC}"
echo -e "${BLUE}Press Ctrl+C to stop${NC}"
echo

# Set environment variables for the relayer
export HYP_BASE_DB="$DB_DIR"
export HYP_CHAINS_XOMARKETTESTNET_SIGNER_KEY="$XO_SIGNER_KEY"
export HYP_CHAINS_CELESTIATESTNET_SIGNER_KEY="$CELESTIA_SIGNER_KEY"
export HYP_DEFAULTSIGNER_KEY="$XO_SIGNER_KEY"

# Run the relayer (it will automatically load all configs from ./config directory)
"$RELAYER_BINARY"
