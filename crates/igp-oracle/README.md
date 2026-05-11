# igp-oracle

`igp-oracle` is a one-shot dry-run CLI for Hyperlane IGP configuration
reconciliation.

The current implementation starts from configured origin IGPs, discovers
cosmosnative destination gas configs through the Hyperlane protobuf gRPC query
service, resolves discovered remote domains through the local Hyperlane
registry, fetches market and gas data, computes proposed IGP values, compares
deltas, and writes review artifacts.

It does not sign transactions, submit transactions, or send notifications.

## Build

From the repository root:

```bash
cargo build -p igp-oracle
```

Run through Cargo:

```bash
cargo run -p igp-oracle -- reconcile --help
```

Run the built binary directly:

```bash
target/debug/igp-oracle reconcile --help
```

## Configuration

The example config is:

```text
crates/igp-oracle/igp-oracle.example.yaml
```

Configured targets use:

```yaml
remoteSelection: configuredOnOriginIgp
```

That means the origin IGP is the source of truth for remote domains. The CLI
then applies optional runtime filters before fetching gas and CoinGecko pricing
data.

Cosmosnative origins can enumerate configured destination gas configs through
the Hyperlane gRPC query service. EVM IGP contracts store destination configs in
Solidity mappings, which are not enumerable through ordinary contract calls.
For EVM origins, use one of these operator-provided selection paths:

```yaml
remoteSelection:
  domains:
    - 1128614981
```

or keep `remoteSelection: configuredOnOriginIgp` and pass exactly one runtime
filter with `--remote-chain <chain>` or `--remote-domain <domain>`.

The config must include market asset IDs for every chain that will be evaluated.
For example:

```yaml
marketData:
  provider: coingecko
  assets:
    celestiatestnet: celestia
    edentestnet: ethereum
    ethereum: ethereum
    arbitrum: ethereum
```

Gas price updates are controlled by `gas.mode`:

```yaml
gas:
  mode: sample
  source: rpc
  min: "1"
  max: "1000000000000"
```

`sample` is the default and recomputes `gasPrice` from the remote gas source.
Use `preserve` when the run should keep the on-chain `gasPrice` and only
evaluate market-driven `tokenExchangeRate` changes:

```yaml
gas:
  mode: preserve
  source: rpc
  min: "1"
  max: "1000000000000"
```

In preserve mode, `igp-oracle` still reads the current IGP config, but it skips
remote gas sampling and sets proposed `gasPrice` equal to current on-chain
`gasPrice`.

## Basic Dry Run

Dry-run is the default behavior. `--dry-run` can be included for clarity.

```bash
cargo run -p igp-oracle -- reconcile \
  --config crates/igp-oracle/igp-oracle.example.yaml \
  --registry . \
  --origin celestiatestnet \
  --output-dir .tmp/igp-oracle \
  --dry-run
```

This discovers the configured remote domains on the `celestiatestnet` origin
IGP, skips domains that cannot be resolved through the local registry, evaluates
the remaining targets, and writes artifacts to `.tmp/igp-oracle`.

## Filter to One Remote Chain

Use `--remote-chain` to evaluate only one resolved remote chain from the origin
sweep.

```bash
cargo run -p igp-oracle -- reconcile \
  --config crates/igp-oracle/igp-oracle.example.yaml \
  --registry . \
  --origin celestiatestnet \
  --remote-chain edentestnet \
  --output-dir .tmp/igp-oracle-celestiatestnet-edentestnet \
  --dry-run
```

This is usually the best development command because it keeps external pricing
and gas requests small.

## Filter to One Remote Domain

Use `--remote-domain` when you know the Hyperlane domain ID.

```bash
cargo run -p igp-oracle -- reconcile \
  --config crates/igp-oracle/igp-oracle.example.yaml \
  --registry . \
  --origin celestiatestnet \
  --remote-domain 2147483647 \
  --output-dir .tmp/igp-oracle-domain \
  --dry-run
```

For cosmosnative origins, `--remote-chain` and `--remote-domain` are filters
over the origin IGP sweep. For EVM origins, they provide the operator-selected
domain to read directly.

## Celestia Mainnet Example

To evaluate Celestia mainnet, the config must contain a target with:

```yaml
targets:
  - originChain: celestia
    remoteSelection: configuredOnOriginIgp
```

It must also include a market asset entry for `celestia`.

Once the config includes those entries, a filtered mainnet run looks like:

```bash
cargo run -p igp-oracle -- reconcile \
  --config crates/igp-oracle/igp-oracle.example.yaml \
  --registry . \
  --origin celestia \
  --remote-chain ethereum \
  --output-dir .tmp/igp-oracle-celestia-ethereum \
  --dry-run
```

For cosmosnative origins, the registry chain metadata must include a reachable
`grpcUrls` endpoint. The CLI uses gRPC/protobuf queries directly; it does not
shell out to `celestia-appd`.

## Generate-Only Write Plan

`--write --generate-only` recomputes reconciliation from live data and then
adds a top-level `writePlan` section to `igp-plan.json`. It does not sign,
submit, or require private keys.

```bash
cargo run -p igp-oracle -- reconcile \
  --config crates/igp-oracle/igp-oracle.example.yaml \
  --registry . \
  --origin celestiatestnet \
  --remote-chain edentestnet \
  --output-dir .tmp/igp-oracle-write-plan \
  --write \
  --generate-only
```

Cosmosnative generate-only mode can group multiple
`MsgSetDestinationGasConfig` messages for one origin into a single transaction
model. EVM generate-only mode is intentionally narrower: it supports exactly one
remote target and emits one `StorageGasOracle.setRemoteGasData` calldata payload.

Generate-only also validates signer configuration before marking a write plan
ready. EVM plans require the configured signer address to match the
`StorageGasOracle.owner()` address. Cosmosnative plans validate the signer
profile and protocol; when `from` is a local key alias rather than a bech32
address, the artifact records `key_alias_unverified`.

## Cosmosnative Write Submission

`--write` without `--generate-only` signs and submits a cosmosnative
`MsgSetDestinationGasConfig` through `celestia-grpc`. The signer private key is
loaded from the configured signer profile `keyEnv`. The value may include a
`0x` prefix, but the key is never written to artifacts.

```bash
export HYP_KEY_COSMOSNATIVE=0x...

cargo run -p igp-oracle -- reconcile \
  --config crates/igp-oracle/igp-oracle.example.yaml \
  --registry . \
  --origin celestiatestnet \
  --remote-chain edentestnet \
  --output-dir .tmp/igp-oracle-write \
  --write
```

Initial submit mode supports one cosmosnative update target at a time. Use
`--remote-chain` or `--remote-domain` to select a single remote. Generate-only
mode may still model multiple cosmosnative messages for review.

## Artifacts

Each run writes two files:

```text
<output-dir>/
  igp-summary.md
  igp-plan.json
```

`igp-plan.json` is the canonical machine-readable artifact. It includes:

- run policy
- discovery results
- skipped domains
- current on-chain values
- gas inputs, including raw amount/denom, sampled integer gas price, proposed gas price, and rounding details
- market price inputs used for `tokenExchangeRate`
- proposed values
- deltas
- decision status and code
- review-only transaction plan data when available
- transaction planning errors when evaluation succeeds but tx payload construction fails
- generate-only write grouping metadata when `--write --generate-only` is used
- signer authorization status for write targets
- write receipts when a transaction is submitted

`igp-summary.md` is the human-readable review artifact intended for GitHub step
summaries and notification bodies.

`tx-plan.json` is not emitted by default. Transaction plan data lives on each
target in `igp-plan.json`.

For EVM origins, dry-run transaction planning emits review-only calldata for
`StorageGasOracle.setRemoteGasData((uint32,uint128,uint128))`. This updates the
remote token exchange rate and remote gas price only. The IGP destination gas
oracle address and gas overhead are preserved; gas overhead write planning is
intentionally not implemented yet.

For cosmosnative remotes, registry gas prices may be fractional, while on-chain
IGP `gasPrice` is an integer. The plan records the raw value, the rounded sample,
and the rounding reason so `tokenExchangeRate` can remain a meaningful
market-derived value.

## Exit Codes

- `0`: dry-run completed with no required updates, `--write --generate-only`
  completed and wrote a ready or no-op write plan, or a supported write
  completed
- `10`: at least one target has `update_recommended`
- `20`: config, registry, market data, gas data, artifact, or on-chain read error
- `30`: target selection, unsupported protocol, or policy violation
- `40`: unsupported write path

## Current Limitations

- Cosmosnative origin reads are implemented through gRPC/protobuf.
- EVM origin reads and dry-run tx planning are supported only for
  operator-selected domains. Full EVM origin sweep discovery is not supported
  because standard mapping-based IGP contracts do not expose enumerable
  configured domains.
- EVM tx planning currently covers `StorageGasOracle.setRemoteGasData` only.
  Gas overhead and gas oracle address updates are not planned yet.
- EVM write submission is not implemented yet.
- Cosmosnative submit mode supports one update target at a time. Multi-message
  cosmosnative submission is still modeled in generate-only output only.
- Slack, IM, and webhook notifications are workflow-owned. `igp-oracle` only
  emits artifacts, logs, and exit codes.
