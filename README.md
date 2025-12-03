# hyperlane-ops

A repository that hosts Hyperlane deployment configurations and documentation for Celestia Mocha and Eden testnets.

## Prerequisites

TODO: Fill out prereqs for installing

- Docker (link)
- Hyperlane CLI (link)

## Commands

Currently a section to dump commands used to build up this repository.

Reading a core config from Eden.
```bash
hyperlane core read --chain edentestnet --config configs/eden-core.yaml --registry registry
```

Reading a core config from Celestia.
```bash
hyperlane core read --chain celestiamocha --config configs/mocha-core.yaml --registry registry
```

Reading a warp config from Celestia.
```bash
hyperlane warp read --registry ./registry --config mocha-warp.yaml --chain celestiamocha --address 0x726f757465725f61707000000000000000000000000000010000000000000006
```
