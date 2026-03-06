---
name: relayer-ops
description: Use this skill to operate, verify, and troubleshoot Hyperlane relayer services and monitoring in this repository.
---

# Relayer Ops Skill

## Source of truth
- `relayer/config.json`
- `docker-compose.yml`
- `docs/relayer.md`
- `monitoring/prometheus/prometheus.yml`
- `monitoring/grafana/dashboards/*.json`

## Read-only checks first
```bash
docker compose ps
docker logs hyperlane-relayer --tail=200
curl -fsS http://127.0.0.1:9090/metrics | head
```

## Mutating commands (approval required)
```bash
docker compose restart relayer
docker compose up -d relayer
docker compose down -v
```

## Validation checklist
- `relayChains` matches configured chain entries.
- Metrics endpoint is reachable and scraped by Prometheus.
- Grafana datasource points to Prometheus container service URL.
