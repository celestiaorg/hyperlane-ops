# Eden Testnet Faucets

This directory contains the PoW faucet deployment configuration for the Eden testnet.
The Docker Compose stack launches four independent faucet instances, each serving a different asset.

## Supported Faucets
- TIA (native token): http://51.159.182.223:8082/
- ETH (ERC-20): http://51.159.182.223:8083/
- LBTC (ERC-20): http://51.159.182.223:8081/
- USDC (ERC-20): http://51.159.182.223:8080/

Each faucet runs as its own service with isolated configuration and state.

## Requirements
- Docker

## Running the Faucets
Start all faucet services in the background:
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
Each faucet is configured via its own mounted data directory. This includes:
- faucet configuration files
- runtime state
- databases and logs

Token parameters (contract addresses, payout amounts, thresholds, etc.) are defined per faucet and should be updated in their respective configuration directories.
