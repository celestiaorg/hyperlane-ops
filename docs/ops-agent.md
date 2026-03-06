# Ops Agent

The ops-agent is a Pi-based orchestration service for Hyperlane core deployment, warp deployment, and relayer operations.

## Highlights

- Runs in Docker Compose under the `agent` profile.
- Exposes a local HTTP API on `127.0.0.1:8787`.
- Uses two-step approval for mutating commands.
- Exposes Prometheus metrics at `/metrics`.

## Start

```bash
docker compose --profile agent up -d --build ops-agent
```

## API

```bash
curl -sS -X POST http://127.0.0.1:8787/v1/plan \
  -H 'content-type: application/json' \
  -d '{"goal":"check relayer health"}'
```

```bash
curl -sS -X POST http://127.0.0.1:8787/v1/approve \
  -H 'content-type: application/json' \
  -d '{"planId":"<plan>","commandHashes":["<hash>"],"ttlSeconds":900}'
```

```bash
curl -sS -X POST http://127.0.0.1:8787/v1/execute \
  -H 'content-type: application/json' \
  -d '{"planId":"<plan>","approvalToken":"<token>"}'
```

## Environment

- `PI_DEFAULT_PROVIDER`
- `PI_DEFAULT_MODEL`
- `HYP_KEY`
- `HYP_KEY_COSMOSNATIVE`

## Metrics

The following counters are exported:

- `plans_created_total`
- `writes_blocked_total`
- `writes_executed_total`
- `policy_violation_total`
- `run_failures_total`
