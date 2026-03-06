---
name: hyperlane-core
description: Use this skill for Hyperlane core deploy/apply/read workflows on EVM and Celestia chains in this repository.
---

# Hyperlane Core Skill

## Source of truth
- `docs/celestia-core-deploy.md`
- `docs/evm-core-deploy.md`
- `chains/<chain>/metadata.yaml`
- `chains/<chain>/addresses.yaml`
- `configs/*-core.yaml`

## Required safeguards
- Always include `--registry .` in Hyperlane CLI commands.
- Require `HYP_KEY` for EVM mutating commands.
- Require `HYP_KEY_COSMOSNATIVE` for Celestia/cosmosnative mutating commands.
- Never write secrets to files.

## Workflow
1. Preflight: confirm chain exists in registry, config path is correct, signer env vars are set.
2. Deploy or apply:
```bash
hyperlane core deploy --chain <chain> --config configs/<chain>-core.yaml --registry .
hyperlane core apply --chain <chain> --config configs/<chain>-core.yaml --registry .
```
3. Read back and persist artifacts:
```bash
hyperlane core read --chain <chain> --config configs/<chain>-core.yaml --registry .
```
4. Validate `chains/<chain>/addresses.yaml` updates and keep aggregate files aligned.
