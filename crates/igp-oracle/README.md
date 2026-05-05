# igp-oracle

`igp-oracle` is a one-shot dry-run CLI for IGP configuration reconciliation.

The current dry-run resolves targets from the local Hyperlane registry, fetches market and gas data, reads cosmosnative destination gas configs through the Hyperlane protobuf gRPC query service, computes proposed IGP values, compares deltas, and writes review artifacts. It does not sign transactions, submit transactions, perform EVM on-chain reads, or send notifications.

## Current Dry Run

From the repository root:

```bash
cargo run -p igp-oracle -- reconcile \
  --config crates/igp-oracle/igp-oracle.example.yaml \
  --registry . \
  --origin celestiatestnet \
  --remote-chain edentestnet \
  --output-dir /tmp/igp-oracle \
  --dry-run
```

Expected artifacts:

- `/tmp/igp-oracle/igp-summary.md`
- `/tmp/igp-oracle/igp-plan.json`
- `/tmp/igp-oracle/tx-plan.json`

`igp-plan.json` includes proposal inputs, market/gas provenance, the on-chain gRPC query endpoint, current values, proposed values, deltas, a structured decision code, and any review-only transaction plan.

`tx-plan.json` is generated for operator review when a non-`noop` target has enough data to plan a protocol-specific update. For cosmosnative origins this currently models `/hyperlane.core.post_dispatch.v1.MsgSetDestinationGasConfig` but does not sign, generate, or submit a transaction.

`--write` is parsed but intentionally exits with code `40` in stage one.

For cosmosnative origins, the registry chain metadata must include a reachable `grpcUrls` endpoint for current on-chain reads. Dry-runs exit with code `10` when an update is recommended and `30` when a policy limit is exceeded.
