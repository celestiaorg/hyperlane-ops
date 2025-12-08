# Testnet Deployments

The following document contains testnet deployments for the Eden testnet. All assets are bridged via Celestia mocha testnet.

## Warp Routes

It is important to familiarize yourself with the following [Warp Architecture](https://github.com/hyperlane-xyz/hyperlane-monorepo/tree/main/solidity/contracts/token#warp-route-architecture) document.

`Native` - for warping native assets (e.g. ETH) from the canonical chain
`Collateral` - for warping tokens, ERC20 or ERC721, from the canonical chain
`Synthetic` - for representing tokens, Native/ERC20 or ERC721, on a non-canonical chain

Note, that in Hyperlane `cosmosnative` chains, only the `Collateral` and `Synthetic` token types are supported.

### TIA

The following is a `HypERC20` synthetic token deployment on the Edentest. See below for details on native `utia` route.

| Token Type | TokenID/Address                                                      | Chain (Domain)                | 
| ---------- | -------------------------------------------------------------------- | ----------------------------- |
| Collateral | `0x726f757465725f6170700000000000000000000000000001000000000000001e` | Celestia Mocha (`1297040200`) |
| Synthetic  | `0xb1F7Bf7E4765CAcc93Fe32A48754314F8B66152e`                         | Eden Testnet (`2147483647`)   |

### Native TIA

> [!WARNING]  
> This route is not live yet as it requires a testnet upgrade as a prerequisite.

> The `utia` asset is deployed with a custom `TokenRouter` implementation. The `HypNativeMinter` contract is used in order to facilitate native `utia` collateral as the canonical chain asset on Eden. This contract is integrated directly with a custom precompile used for native asset minting which by default in EVM environments is counted in units of `wei` (18 decimals).

| Token Type | TokenID/Address                                                    | Chain (Domain)              | 
| ---------- | ------------------------------------------------------------------ | --------------------------- |
| Collateral  | TODO | Celestia Mocha (1297040200) |
| HypNativeMinter | TODO | Eden Testnet (`2147483647`)   |

### Noble USDC

| Token Type | TokenID/Address                                                      | Chain (Domain)                | 
| ---------- | -------------------------------------------------------------------- | ----------------------------- |
| Collateral | `0x726f757465725f61707000000000000000000000000000010000000000000008` | Noble Testnet (`1196573006`)  |
| Synthetic  | `0x726f757465725f61707000000000000000000000000000020000000000000015` | Celestia Mocha (`1297040200`) |
| Synthetic  | `0xe1141469cff3db185a0e1b9ebd1d31bb22a623ea`                         | Eden Testnet (`2147483647`)   |

### Sepolia ETH

| Token Type | TokenID/Address                                                      | Chain (Domain)                | 
| ---------- | -------------------------------------------------------------------- | ----------------------------- |
| Native     | `0xEEea7Edeb303A1D20F3742edfC66F188f805a28E`                         | Sepolia Testnet (`11155111`)  |
| Synthetic  | `0x726f757465725f6170700000000000000000000000000002000000000000001d` | Celestia Mocha (`1297040200`) |
| Synthetic  | TODO:``                         | Eden Testnet (`2147483647`)   |

