# Warp Routes Guide

Warp Routes are Hyperlane’s modular token-bridging application. They enable permissionless token transfers across chains using Hyperlane messaging and ISM-based security. Warp Routes support native assets and ERC20 tokens on EVM chains with security configurable per route via ISMs.

## Prerequisites

!!! tip
    Please see [Celestia Core Deployment](./celestia-core-deploy.md) and [EVM Core Deployment](./evm-core-deploy.md) for walkthroughs on setting up Hyperlane core infrastructure.

- Hyperlane core must be deployed on each chain in the route.
- Hyperlane CLI installed.
- A funded deployer key (EVM) exported as `HYP_KEY` (or enter it when prompted).
- A funded deployer key (Celestia) exported as `HYP_KEY_COSMOSNATIVE`.
- Chain metadata exists in the registry (`chains/<chain>/metadata.yaml`).

If a chain is missing from the registry, add it first. To list chains use:
```bash
hyperlane registry list --registry .
```

## Create a Deployment Config
This registry tracks warp deployment configs under `deployments/warp_routes/<TOKEN>/`. 
Please refer to the registry [`deployments/warp_routes`](https://github.com/celestiaorg/hyperlane-ops/tree/main/deployments/warp_routes) directory for example YAML configs.

Warp route configs must confirm to the configuration schema, this can be found in the same directory mentioned above.

For EVM routes, the `type` field typically includes:

- `collateral` for an ERC20/ERC721 token on the canonical chain
- `native` for a native token (e.g., ETH)
- `collateralVault` for ERC4626 vault-backed collateral

!!! note
    For `cosmosnative` chains, the native token is defined as a `collateral` token. For example, `utia`.

Optional fields include the following:

- `symbol`, `name`, `decimals`
- `mailbox`
- `interchainSecurityModule`

!!! tip
    For `cosmosnative` chains, make sure to include `decimals` and `scale` where required by the chain’s token module.

Create a [name]-deploy.yaml file in the correct folder in the registry. Use an existing file as a template. You can remove the `interchainSecurityModule` entry if the default ISM should be used.

```yaml
celestia:
  decimals: 6
  name: TIA
  owner: "celestia1lg0e9n4pt29lpq2k4ptue4ckw09dx0aujlpe4j"
  symbol: TIA
  token: utia
  type: collateral
ethereum:
  decimals: 6
  name: TIA
  owner: "0xc259e540167B7487A89b45343F4044d5951cf871"
  symbol: TIA
  type: synthetic
```

The CLI wizard can be used to generate a config file however `cosmosnative` chains are only _partially_ supported.
The CLI will prompt for network type, chains to connect, token type, and other options. This will produce a YAML file similar to what was described above.

```bash
hyperlane warp init --registry .
```

## Deploy the Warp Route
Run the deployment using the config:

```bash
hyperlane warp deploy \
  --wd ./deployments/warp_routes/<TOKEN>/<route>-deploy.yaml \
  --wc ./deployments/warp_routes/<TOKEN>/<route>-config.yaml \
  --registry .
```

Notes:
!!! note
    - If `--wd`/`--wc` are omitted, the CLI writes artifacts to `$HOME/.hyperlane/deployments/warp_routes/`.
    - The deployer will prompt for the private key if `HYP_KEY` or `HYP_KEY_COSMOSNATIVE` is not set.

After deployment:

- Ensure the output `*-config.yaml` is committed.
- Update `deployments/warp_routes/warpRouteConfigs.yaml` with the new route entry.
- Add or update `deployments/warp_routes/<TOKEN>/logo.svg` if needed.

## Verify and Test
Read the on-chain Warp Route configuration:
```bash
hyperlane warp read --symbol <TOKEN> --registry .
```

Send a test transfer (optional):
```bash
hyperlane warp send --relay --symbol <TOKEN> --registry .
```

## Update or Extend a Route

To change ownership, ISM settings, or add additional chains, update your deployment config and apply it:

```bash
hyperlane warp apply \
  --symbol <TOKEN> \
  --config ./deployments/warp_routes/<TOKEN>/<route>-deploy.yaml \
  --registry .
```

If you add a new chain, ensure the chain metadata exists in the registry and re-run `warp apply`.
