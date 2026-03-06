---
name: incident-triage
description: Use this skill when diagnosing delivery failures, stuck queues, failed transfers, or chain-specific routing issues.
---

# Incident Triage Skill

## Inputs to gather
- affected route/token symbol
- origin/destination chain pair
- approximate failing time window
- whether relayer was healthy at that time

## Investigation flow
1. Check relayer liveness and logs:
```bash
docker compose ps relayer
docker logs hyperlane-relayer --tail=500
```
2. Check route state on-chain:
```bash
hyperlane warp read --symbol <TOKEN> --registry .
```
3. Check manual transfer primitives:
```bash
cast call ...
celestia-appd query warp ...
```
4. Only after read-only diagnosis, propose minimal mutating remediation.

## Guardrails
- Keep diagnostics reproducible and command-based.
- Never mutate chain state without explicit approval.
- Never include private keys in output.
