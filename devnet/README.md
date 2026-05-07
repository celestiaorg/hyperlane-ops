# Devnet Validator Multisig WarpRoute

This runbook covers a local devnet Warp Route setup using a single validator multisig Hyperlane connection for (`celestiadev` <-> `anvil`):

Using the following Warp route:

- `TIA/celestiadev-anvil`

## Prerequisites

- Run commands from the repository root.
- Docker is available locally.
- You have a validator checkpoint key for the Hyperlane validator services.

This devnet was validated with the address `0x122644796671D1D90B20bA291C5081625e298059` using the key below:

```bash
export HYP_VALIDATOR_CHECKPOINT_KEY=0x59c6995e998f97a5a0044966f094538e9e86dae88c7a8412f4603b6b78690d1b
```

## Running a local Docker network

Start the devnet:

```bash
make -C devnet start HYP_VALIDATOR_CHECKPOINT_KEY="$HYP_VALIDATOR_CHECKPOINT_KEY"
```

Check service status:

```bash
make -C devnet ps
```

Follow logs:

```bash
make -C devnet logs
```

Stop containers and remove volume state:

```bash
make -C devnet stop
```

## Route Constants

The `Makefile` already carries the working defaults for this devnet, including:

- `ANVIL_DOMAIN=1234`
- `CELESTIADEV_DOMAIN=69420`
- `ANVIL_TIA_ROUTER=0xa85233C63b9Ee964Add6F2cffe00Fd84eb32338f`
- `CELESTIADEV_TIA_TOKEN_ID=0x726f757465725f61707000000000000000000000000000010000000000000000`
- `ANVIL_DEFAULT=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266`
- `CELESTIA_DEFAULT=celestia1d2qfkdk27r2x4y67ua5r2pj7ck5t8n4890x9wy`
- `AMOUNT_UNITS=1000000`

You can override any of them per command if needed.

## 1. Preflight Checks

Verify the Celestia-side Warp token:

```bash
make -C devnet query-celestia-warp-token
```

Verify Celestia has the Anvil router enrolled:

```bash
make -C devnet query-celestia-warp-remotes
```

Verify Anvil has the Celestia router enrolled:

```bash
make -C devnet query-anvil-router
```

Quote the Celestia-origin Hyperlane fee:

```bash
make -C devnet quote-celestia-transfer
```

Expected result:

```json
{"gas_payment":[{"denom":"utia","amount":"36400"}]}
```

Quote the EVM-origin Hyperlane fee:

```bash
make -C devnet quote-anvil-transfer
```

Expected result on this devnet is a non-zero `uint256` payment in Anvil wei
because Anvil uses an interchain gas paymaster as its default hook.

The `transfer-anvil` make target quotes this value and passes it as
`msg.value` automatically.

Check starting balances:

```bash
make -C devnet balance-anvil
make -C devnet balance-celestia
```

On a fresh devnet, the Anvil synthetic balance should be `0` and the Celestia default account starts with `1000000000000utia`.

## 2. Celestia -> Anvil

Send `1 TIA` from `celestiadev` to the Anvil default account:

```bash
make -C devnet transfer-celestia
```

Important:

- The Celestia-origin transfer needed an explicit higher gas limit in this devnet.
- A lower gas limit of `200000` failed with out-of-gas.

Query the tx after broadcast:

```bash
make -C devnet query-celestia-tx TXHASH=<CELESTIA_TX_HASH>
```

The successful tx used during validation was:

```text
98E538ECA0B52FBFD5F8BE1EC39AA0EF8BFC8FAA7D2955E243F1C666CD34EB87
```

Expected success signals:

- `code: 0`
- `hyperlane.core.v1.EventDispatch`
- `hyperlane.core.post_dispatch.v1.EventInsertedIntoTree`
- `hyperlane.core.post_dispatch.v1.EventGasPayment`

Watch the validator and relayer:

```bash
make -C devnet logs-validator-celestiadev
make -C devnet logs-relayer
```

You should see the Celestia validator ingest the first leaf and the relayer eventually process the message.

Verify the Anvil synthetic balance:

```bash
make -C devnet balance-anvil
```

Expected result:

```text
1000000
```

## 3. Anvil -> Celestia Return Redemption

For the return leg, the EVM recipient must be the raw 20-byte Celestia account bytes left-padded to `bytes32`.

If you need to derive that value for a different Celestia account:

```bash
make -C devnet debug-celestia-addr ADDR=<CELESTIA_BECH32_ADDRESS>
```

For the devnet `default` account, the raw hex is:

```text
0x6A809B36CAF0D46A935EE76835065EC5A8B3CEA7
```

Send the synthetic token back to Celestia:

```bash
make -C devnet transfer-anvil
```

The validated return tx was:

```text
0x4ec90b1bc4727b29f5ecba39e91ce3dfb07c22242022ce27c81d9b2b8f5b8c8b
```

Expected success signals on Anvil:

- ERC20 burn log for `1000000`
- mailbox dispatch log
- merkle tree insertion log

Watch the Anvil validator and relayer:

```bash
make -C devnet logs-validator-anvil
make -C devnet logs-relayer
```

The relayer should identify the 1-of-1 validator set and build multisig metadata for destination `celestiadev`.

Verify final balances:

```bash
make -C devnet balance-anvil
make -C devnet balance-celestia
```

Expected results after the round trip:

- Anvil synthetic balance returns to `0`
- Celestia balance increases by `1000000utia` relative to its post-bridge balance

On the validated run, the Celestia default account balance returned to:

```text
999999918600 utia
```
