# igp-oracle

`igp-oracle` is a one-shot dry-run CLI for IGP configuration reconciliation.

The current dry-run resolves targets from the local Hyperlane registry, fetches market and gas data, computes proposed IGP values, selects protocol-specific adapter skeletons, and writes review artifacts. It does not perform live on-chain IGP reads, transaction generation, transaction submission, or notifications.

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

`--write` is parsed but intentionally exits with code `40` in stage one.
