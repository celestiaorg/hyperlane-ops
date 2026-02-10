# Warp Routes Multichain Setup

## Example: Creating an ERC20 Collateral Token Using Three Chains

Run the following commands and follow the interactive prompts.

```bash
hyperlane warp init --registry .
```

Selections used in this example:

- Three chains: `celestiatestnet`, `edentestnet`, `sepolia`
- Synthetic token type for both Celestia testnet and Eden testnet
- Collateral token for Sepolia Ethereum testnet with ERC20 token address
- Explicitly override the account address when interacting with `cosmosnative` modules

A deployment YAML file will be created similar to the following. If your ERC20 uses custom decimals, update them manually in the generated YAML:

```diff
celestiatestnet:
+  decimals: 6
+  scale: 100
  isNft: false
  owner: "celestia1lg0e9n4pt29lpq2k4ptue4ckw09dx0aujlpe4j"
  type: synthetic
edentestnet:
+  decimals: 8
  isNft: false
  owner: "0xc259e540167B7487A89b45343F4044d5951cf871"
  type: synthetic
sepolia:
+  decimals: 8
  owner: "0xc259e540167B7487A89b45343F4044d5951cf871"
  token: "0x0A3eC97CA4082e83FeB77Fa69F127F0eAABD016E"
  type: collateral
```

Deploy the warp route:

```bash
hyperlane warp deploy --wd ./deployments/warp_routes/LBTC/celestiatestnet-edentestnet-sepolia-deploy.yaml --wc ./deployments/warp_routes/ETH/celestiatestnet-edentestnet-sepolia-config.yaml --registry .
```

## Example: Extending an Existing Warp Route

Add Eden testnet as a new synthetic token on an existing warp route between Celestia Mocha and Sepolia.
Note that the following steps automatically enroll a remote router on Eden that connects directly back to Sepolia.

If you want to force users to go back to Sepolia via Celestia Mocha, you must manually unenroll that remote router using `cast` with the associated owner account.

1. Extend the deploy config file to add `edentestnet`.

```diff
sepolia:
  decimals: 18
  name: ETH
  owner: "0xc259e540167B7487A89b45343F4044d5951cf871"
  symbol: ETH
  type: native
celestiatestnet:
  decimals: 6
  scale: 1000000000000 # 10^12
  name: ETH
  owner: "celestia1lg0e9n4pt29lpq2k4ptue4ckw09dx0aujlpe4j"
  symbol: ETH
  token: ETH
  type: synthetic
+ edentestnet:
+   decimals: 18
+   name: ETH
+   owner: "0xc259e540167B7487A89b45343F4044d5951cf871"
+   symbol: ETH
+   token: ETH
+   type: synthetic
```

2. Apply the updated config:

```bash
hyperlane warp apply --symbol ETH --config ./deployments/warp_routes/ETH/celestiatestnet-edentestnet-sepolia-deploy.yaml --registry .
```
