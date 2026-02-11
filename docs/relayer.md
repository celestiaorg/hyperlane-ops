# Relayer

!!! warning
    The following page is work-in-progress and must be updated to include valuable content.

The relayer runs as a Docker container using the repository root `docker-compose.yml`.

## Configuration
- `relayer/config.json` is the Hyperlane agent configuration.
- Keep `relayChains` in sync with chain entries in the registry.

## Run
From the repo root:
```bash
docker compose up -d
```

Check status:
```bash
docker compose ps
```

View logs:
```bash
docker compose logs -f
```
