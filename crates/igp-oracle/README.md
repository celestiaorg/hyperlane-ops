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

`--remote-chain` and `--remote-domain` are filters over the origin IGP sweep.
They do not replace `remoteSelection: configuredOnOriginIgp`.

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

`igp-summary.md` is the human-readable review artifact intended for GitHub step
summaries and notification bodies.

`tx-plan.json` is not emitted by default. Transaction plan data lives on each
target in `igp-plan.json`.

For cosmosnative remotes, registry gas prices may be fractional, while on-chain
IGP `gasPrice` is an integer. The plan records the raw value, the rounded sample,
and the rounding reason so `tokenExchangeRate` can remain a meaningful
market-derived value.

## Exit Codes

- `0`: dry-run completed with no required updates
- `10`: at least one target has `update_recommended`
- `20`: config, registry, market data, gas data, artifact, or on-chain read error
- `30`: target selection, unsupported protocol, or policy violation
- `40`: `--write` was requested

`--write` is parsed but intentionally unsupported at this stage.

## Current Limitations

- Cosmosnative origin reads are implemented through gRPC/protobuf.
- EVM origin reads are still limited.
- EVM origin sweep discovery is not supported yet because standard mapping-based
  IGP contracts do not expose enumerable configured domains.
- Slack, IM, and webhook notifications are workflow-owned. `igp-oracle` only
  emits artifacts, logs, and exit codes.
