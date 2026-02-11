# Relayer 

## Operating a relayer

This repository contains a `docker-compose.yml` file with a Hyperlane relayer agent configuration maintained and mounted for the container from the `relayer/config.json` file.

This relayer agent may deployed on a server for supporting Hyperlane connection and warp route liveness.

### Usage

The following is a set of useful commands for operating the Hyperlane relayer agent.

1. Start the docker compose service in the background.
```bash
docker compose up -d
```

2. Stop the docker compose service and delete the associated volume data.
```bash
docker compose down -v
```

3. Restart the relayer container.
```bash
docker compose restart relayer
```

4. Output and stream the relayer container logs. Optionally provide the `--tail=0` flag to stream latest and discard historical logs.
```bash
docker logs hyperlane-relayer -f
```

If operating the relayer agent on a server for an extended period of time, you may require a large amount of disk space in order to keep it alive.
The relayer agent can consume alot of disk space by the JSON logs alone. You can inspect this and truncate the log file using the following commands. If the relayer agent stopped working due to this, a container restart may be required.

1. Inspect JSON log file size.
```bash
sudo du -sh /var/lib/docker/containers/*/*-json.log 2>/dev/null | sort -h | tail
```

2. Truncate the JSON log file using the container ID from the first command.
```bash
sudo truncate -s 0 /var/lib/docker/containers/<container-id>/<container-id>-json.log
```