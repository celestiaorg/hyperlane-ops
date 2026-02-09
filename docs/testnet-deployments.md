# Testnet Deployments

The following document contains testnet deployments for the Eden testnet. All assets are bridged via Celestia Mocha testnet.

## Warp Routes

It is important to familiarize yourself with the [Warp Architecture document](https://github.com/hyperlane-xyz/hyperlane-monorepo/tree/main/solidity/contracts/token#warp-route-architecture).

Token types:
- `Native` for warping native assets (e.g. ETH) from the canonical chain
- `Collateral` for warping tokens (ERC20 or ERC721) from the canonical chain
- `Synthetic` for representing tokens on a non-canonical chain

Note: In Hyperlane `cosmosnative` chains, only `Collateral` and `Synthetic` token types are supported.

## Native TIA

The `utia` asset is deployed with a custom `TokenRouter` implementation. The `HypNativeMinter` contract is used to facilitate native `utia` collateral as the canonical chain asset on Eden. This contract integrates with a custom precompile used for native asset minting which defaults to 18 decimals in EVM environments.

A native TIA faucet is available on Eden Testnet [here](http://51.159.182.223:8083/).

| Token Type      | TokenID/Address                                                      | Chain (Domain)                |
| --------------- | -------------------------------------------------------------------- | ----------------------------- |
| Collateral      | `0x726f757465725f6170700000000000000000000000000001000000000000001a` | Celestia Mocha (`1297040200`) |
| HypNativeMinter | `0x43505da95A74Fa577FB9bB0Ce29E293FdF575011`                         | Eden Testnet (`2147483647`)   |

## ERC20 TIA

!!! warning
    Deprecated: Please use the canonical native TIA route specified above.

The following is a `HypERC20` synthetic token deployment on Edentest. See above for details on the native `utia` route.

| Token Type | TokenID/Address                                                      | Chain (Domain)                |
| ---------- | -------------------------------------------------------------------- | ----------------------------- |
| Collateral | `0x726f757465725f6170700000000000000000000000000001000000000000001e` | Celestia Mocha (`1297040200`) |
| Synthetic  | `0xb1F7Bf7E4765CAcc93Fe32A48754314F8B66152e`                         | Eden Testnet (`2147483647`)   |

## Noble USDC

A USDC faucet is available on Eden Testnet [here](http://51.159.182.223:8080/).

| Token Type | TokenID/Address                                                      | Chain (Domain)                |
| ---------- | -------------------------------------------------------------------- | ----------------------------- |
| Collateral | `0x726f757465725f61707000000000000000000000000000010000000000000008` | Noble Testnet (`1196573006`)  |
| Synthetic  | `0x726f757465725f61707000000000000000000000000000020000000000000015` | Celestia Mocha (`1297040200`) |
| Synthetic  | `0xe1141469cff3db185a0e1b9ebd1d31bb22a623ea`                         | Eden Testnet (`2147483647`)   |

## Sepolia ETH

An ETH faucet is available on Eden Testnet [here](http://51.159.182.223:8083/).

| Token Type | TokenID/Address                                                      | Chain (Domain)                |
| ---------- | -------------------------------------------------------------------- | ----------------------------- |
| Native     | `0xEEea7Edeb303A1D20F3742edfC66F188f805a28E`                         | Sepolia Testnet (`11155111`)  |
| Synthetic  | `0x726f757465725f6170700000000000000000000000000002000000000000001d` | Celestia Mocha (`1297040200`) |
| Synthetic  | `0xf8e7A4608AE1e77743FD83549b36E605213760b6`                         | Eden Testnet (`2147483647`)   |

## Sepolia LBTC

The LBTC ERC20 token contract on Ethereum Sepolia is:
`0x0A3eC97CA4082e83FeB77Fa69F127F0eAABD016E`

An LBTC faucet is available on Eden Testnet [here](http://51.159.182.223:8081/).

| Token Type | TokenID/Address                                                      | Chain (Domain)                |
| ---------- | -------------------------------------------------------------------- | ----------------------------- |
| Collateral | `0x101612E45d8D1ebE8e2EB90373b7cCecB6F52F5C`                         | Sepolia Testnet (`11155111`)  |
| Synthetic  | `0x726f757465725f6170700000000000000000000000000002000000000000001f` | Celestia Mocha (`1297040200`) |
| Synthetic  | `0x4d46424A8AA50e7c585F218338BCCE4a9a992c0F`                         | Eden Testnet (`2147483647`)   |
