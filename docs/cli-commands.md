# CLI Commands

This page captures useful CLI commands used to operate this repository.

## Read Core Configs
Reading a core config from Eden:
```bash
hyperlane core read --chain edentestnet --config configs/eden-core.yaml --registry .
```

Reading a core config from Celestia:
```bash
hyperlane core read --chain celestiatestnet --config configs/mocha-core.yaml --registry .
```

## Read Warp Configs
Reading a warp config from Celestia:
```bash
hyperlane warp read --registry . --config mocha-warp.yaml --chain celestiatestnet --address 0x726f757465725f61707000000000000000000000000000010000000000000006
```
