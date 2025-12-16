# Eden Testnet Faucets

This directory contains the **PowFaucet deployment configuration** for the **Eden testnet**.  
The docker compose stack launches **four independent faucet instances**, each serving a different asset.

## Supported Faucets

The Docker Compose stack deploys the following faucets:

- **TIA (native token)**
- **ETH (ERC-20)**
- **LBTC (ERC-20)**
- **USDC (ERC-20)**

Each faucet runs as its own service with isolated configuration and state.

---

## Requirements

- Docker

Make sure Docker is running before proceeding.

---

## Running the Faucets

To start all faucet services in the background, run:

```bash
docker compose up -d
```

View running containers:

```bash
docker compose ps
```

View logs:

```bash
docker compose logs -f
```

Stop and remove containers:

```bash
docker compose down
```

## Configuration

Each faucet is configured via its own mounted data directory.
This includes:
- faucet configuration files,
- runtime state,
- databases and logs.

Token parameters (contract addresses, payout amounts, thresholds, etc.) are defined per faucet and should be updated in their respective configuration directories.
