# Ops Agent

Pi-based orchestration service for Hyperlane core, warp deployment, and relayer operations.

## Features

- Pi-backed command planning via RPC (`pi --mode rpc`)
- Policy gating for mutating actions
- Two-step approval (`plan` -> `approve` -> `execute`)
- Run transcript storage and retrieval
- Prometheus metrics endpoint

## Environment

- `PI_DEFAULT_PROVIDER` and `PI_DEFAULT_MODEL` set provider/model for Pi RPC process.
- `HYP_KEY` required for EVM mutating commands.
- `HYP_KEY_COSMOSNATIVE` required for cosmosnative mutating commands.

## API

- `POST /v1/plan`
- `POST /v1/approve`
- `POST /v1/execute`
- `GET /v1/plans/:id`
- `GET /v1/runs/:id`
- `GET /v1/runs/:id/events`
- `GET /healthz`
- `GET /metrics`

## Local development

```bash
cd ops-agent
npm install
npm run build
npm run test
npm run dev
```

## CLI examples

```bash
cd ops-agent
npm run build
node dist/cli.js plan "check relayer health"
node dist/cli.js run "diagnose relayer lag" --read-only
```
