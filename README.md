# hyperlane-ops

This repository hosts the Celestia Hyperlane registry, containing configurations, operational guides and documentation.

This registry follows a similar structure to the [official Hyperlane registry](https://github.com/hyperlane-xyz/hyperlane-registry) in order to maintain compatibility and alignment with tooling.

## Prerequisites

- Install [Docker](https://www.docker.com/get-started/) for running the Relayer [docker-compose.yml](./docker-compose.yml).
- Install the official [Hyperlane CLI ](https://docs.hyperlane.xyz/docs/reference/developer-tools/cli).
- Install the [`celestia-appd`](https://github.com/celestiaorg/celestia-app) CLI binary.

## Documentation

Full documentation is available at [celestia.org.github.io/hyperlane-ops](https://celestiaorg.github.io/hyperlane-ops/).

The website is built using [mkdocs](https://www.mkdocs.org/) and served by GitHub pages.

### Running the documentation site locally

- Install Python 3 and `mkdocs-material` for local docs preview.

```bash
python3 -m venv .venv
source .venv/bin/activate
pip install mkdocs-material
mkdocs serve
```

Open `http://127.0.0.1:8000/` in your browser.

## Repository Structure

This repository contains a Hyperlane registry with chain configs and canonical addresses used by CLI commands.

- `chains/` contains chain metadata configuration files and addresses for operating on Hyperlane chains.
- `configs/` contains deployment YAMLs for core and warp routes, these are artifacts read from on-chain data.
- `deployments/warp_routes` contains Hyperlane warp route deployment configurations.
- `docs` contains markdown content for the documentation website.
- `faucets/` contains pow-faucet deployments for the Eden-Mocha testnet.
- `relayer/` contains a Hyperlane agent configuration used for operating an off-chain message relayer.
- `solidity/` contains contracts used for native asset minting via Hyperlane.

## Testnet Deployments

Please refer to the [documentation website](https://celestiaorg.github.io/hyperlane-ops/testnet-deployments/) for detailed information about testnet Warp route deployments.

## References

- [Hyperlane Cosmos Runbook](https://hyperlanexyz.notion.site/Runbook-Hyperlane-Cosmos-SDK-2b06d35200d681f2a3c0e481a45b9275)
- [Hyperlane Docs](https://docs.hyperlane.xyz/)
