# Testnet Deployments

## Warp Routes

It is important to familiarize yourself with the following [Warp Architecture](https://github.com/hyperlane-xyz/hyperlane-monorepo/tree/main/solidity/contracts/token#warp-route-architecture) document.

In particular, the various token types used in the tables below. 

`Native` - for warping native assets (e.g. ETH) from the canonical chain
`Collateral` - for warping tokens, ERC20 or ERC721, from the canonical chain
`Synthetic` - for representing tokens, Native/ERC20 or ERC721, on a non-canonical chain

Note, that in Hyperlane `cosmosnative` chains, only the `Collateral` and `Synthetic` token types are supported.

### Native TIA

TODO: This route needs to be deployed as soon as the upgrade is live.

The native `utia` asset is deployed with a custom `TokenRouter` implementation. The `HypNativeMinter` contract is used in order to facilitate native `utia` collateral as the canonical chain asset on Eden. This contract is integrated directly with a custom precompile used for native asset minting which by default in EVM environments is counted in units of `wei` (18 decimals).

| Token Type | TokenID/Address                                                    | Chain (Domain)              | 
| ---------- | ------------------------------------------------------------------ | --------------------------- |
| Collateral  | TODO | Celestia Mocha (1297040200) |
| HypNativeMinter | TODO | Eden Testnet (1297040200)   |

### Noble USDC

| Token Type | TokenID/Address                                                    | Chain (Domain)              | 
| ---------- | ------------------------------------------------------------------ | --------------------------- |
| Collateral | 0x726f757465725f61707000000000000000000000000000010000000000000008 | Noble Testnet (1196573006)  |
| Synthetic  | 0x726f757465725f61707000000000000000000000000000020000000000000015 | Celestia Mocha (1297040200) |
| Synthetic  | 0xe1141469cff3db185a0e1b9ebd1d31bb22a623ea                         | Eden Testnet (1297040200)   |
