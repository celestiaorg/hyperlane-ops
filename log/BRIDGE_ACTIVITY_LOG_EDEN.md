# Bridge Activity Log - Eden ↔ Celestia

This document tracks successful bridge transfers between Eden Testnet and Celestia Mocha-4 Testnet.

## Active Warp Routes

### 1. EDEN Native Token Bridge
**Deployed:** October 22, 2025

**Route Details:**
- **Eden (Native)**: `0x954F1C87a6bc9d102CD4dC85e323500093f793ae`
  - Type: EvmHypNative (wraps native EDEN)
  - Decimals: 18
- **Celestia (Synthetic)**: `0x726f757465725f61707000000000000000000000000000020000000000000007`
  - Type: CosmosNativeHypSynthetic
  - Denom: `hyperlane/0x726f757465725f61707000000000000000000000000000020000000000000007`
  - Decimals: 18

**Domain Configuration:**
- Eden Domain: `2147483647`
- Celestia Domain: `1297040200`

### 2. TIA Token Bridge
**Deployed:** October 22, 2025 (from previous deployment)

**Route Details:**
- **Celestia (Collateral)**: `0x726f757465725f61707000000000000000000000000000010000000000000006`
  - Type: CwHypCollateral (locks native TIA/utia)
  - Decimals: 6
- **Eden (Synthetic)**: `0xcC79A8081D8cFd3da61dE7a25Bc06CCe00BbDa54`
  - Type: EvmHypSynthetic (mints synthetic TIA)
  - Decimals: 6

**Domain Configuration:**
- Eden Domain: `2147483647`
- Celestia Domain: `1297040200`

---

## Transfer History

### TIA Token Transfers

#### Transfer #1: Celestia → Eden
- **Date:** October 22, 2025
- **Transaction Hash (Eden):** `0x61f8eec449a32d34f0a07a2b465efff2f2960a92d74935400acccc8d6f2f29a1`
- **Block Number:** `7453328`
- **Amount:** `1.0 TIA` (1000000 utia)
- **To (Eden):** `0xc259e540167B7487A89b45343F4044d5951cf871`
- **Status:** ✅ Success
- **Type:** Mint (synthetic TIA on Eden)

#### Transfer #2: Celestia → Eden
- **Date:** October 22, 2025
- **Transaction Hash (Eden):** `0x4fd1366ca93e8e5dd6520e4c4afa8209fdd1d988e0f53984b07ebdaf56b635f7`
- **Block Number:** `7453330`
- **Amount:** `0.5 TIA` (500000 utia)
- **To (Eden):** `0xc259e540167B7487A89b45343F4044d5951cf871`
- **Status:** ✅ Success
- **Type:** Mint (synthetic TIA on Eden)

#### Transfer #3: Celestia → Eden
- **Date:** October 22, 2025
- **Transaction Hash (Eden):** `0x02a54e8694457232551ac2c56619ebc45bc73f93a331efe866c203a7ffd049cb`
- **Block Number:** `7453340`
- **Amount:** `0.1 TIA` (100000 utia)
- **To (Eden):** `0xc259e540167B7487A89b45343F4044d5951cf871`
- **Status:** ✅ Success
- **Type:** Mint (synthetic TIA on Eden)

#### Transfer #4: Eden → Celestia
- **Date:** October 22, 2025
- **Transaction Hash (Eden):** `0x53279c8ab2f44960b66d537fa830ed4afcc33726b9617090bd01f0c2f9df14c5`
- **Block Number:** `7453517`
- **Amount:** `0.1 TIA` (100000 utia)
- **From (Eden):** `0xc259e540167B7487A89b45343F4044d5951cf871`
- **Status:** ✅ Success
- **Type:** Burn (synthetic TIA on Eden, unlock on Celestia)

### EDEN Token Transfers

#### Transfer #1: Eden → Celestia
- **Date:** October 22, 2025
- **Transaction Hash (Eden):** `0x2d4ab4a3c7e1aa1acb8759daa3421e3974b91ef0ae69767424373b6d363a93ab`
- **Block Number:** `7454687`
- **Amount:** `0.1 EDEN` (100000000000000000 wei)
- **From (Eden):** `0xc259e540167B7487A89b45343F4044d5951cf871`
- **To (Celestia):** `celestia1lg0e9n4pt29lpq2k4ptue4ckw09dx0aujlpe4j` (hex: `0x000000000000000000000000FA1F92CEA15A8BF08156A857CCD71673CAD33FBC`)
- **Status:** ✅ Success
- **Verification:** Synthetic EDEN balance confirmed on Celestia: `100000000000000000` base units

---

## Router Enrollments

### EDEN Native Bridge Routers

**Eden → Celestia Enrollment**
- Transaction: `0x274a43bf76051ca25f0eb99b4706edaabc5235ae045d28d43582a1b644ba4687`
- Remote Domain: `1297040200`
- Remote Router: `0x726f757465725f61707000000000000000000000000000020000000000000007`
- Gas Used: 120234

**Celestia → Eden Enrollment**
- Transaction: `7824F36B63979E3F5D5A7CABB2A7D5EDD4A24FA73143E1553F0D53E3B6A64632`
- Remote Domain: `2147483647`
- Remote Router: `0x000000000000000000000000954F1C87a6bc9d102CD4dC85e323500093f793ae`
- Gas Limit: `1000000000`

### TIA Bridge Routers

**Eden → Celestia Enrollment**
- Transaction: `0x651b52602ff33c77ed34084ece453419f2efe88858dfba678c6825f8ff0473c8`
- Remote Domain: `1297040200`
- Remote Router: `0x726f757465725f61707000000000000000000000000000010000000000000006`
- Gas Used: 120235

**Celestia → Eden Enrollment**
- Transaction: `6E2A53FC03D8D3016407A9B5DED65EC21DC9312A2D2C98A066D0F8F8771E84E6`
- Remote Domain: `2147483647`
- Remote Router: `0x000000000000000000000000cC79A8081D8cFd3da61dE7a25Bc06CCe00BbDa54`
- Gas Limit: `1000000000`

---

## Network Information

### Eden Testnet
- **Chain ID:** `3735928814`
- **Domain ID:** `2147483647`
- **RPC URL:** `https://ev-reth-eden-testnet.binarybuilders.services:8545`
- **Mailbox:** `0xBdEfA74aCf073Fc5c8961d76d5DdA87B1Be2C1b0`
- **MerkleTreeHook:** `0x22379102569dc3fBeA23Dc34e27F52a76c60F034`

### Celestia Mocha-4 Testnet
- **Chain ID:** `mocha-4`
- **Domain ID:** `1297040200`
- **RPC URL:** `http://celestia-mocha-archive-rpc.mzonder.com:26657`
- **Mailbox:** `0x68797065726c616e650000000000000000000000000000000000000000000003`
- **MerkleTreeHook:** `0x726f757465725f706f73745f6469737061746368000000030000000000000005`

---

## Relayer Configuration

**Configuration File:** `config/relayer-config-eden-mocha.json`

**Monitored Chains:**
- `edentestnet` (Domain: 2147483647)
- `celestiatestnet` (Domain: 1297040200)

**Database:** `/tmp/hyperlane-relayer-db-eden`

**Metrics Port:** `9091`

**Run Script:** `run-relayer-eden.sh`

---

## Statistics

### EDEN Bridge
- **Total Transfers:** 1
- **Total Volume (Eden → Celestia):** 0.1 EDEN
- **Total Volume (Celestia → Eden):** 0 EDEN
- **Gas Costs (Deployment):** 0.004483851031386957 EDEN (Eden side), 0.000000000000000381 TIA (Celestia side)

### TIA Bridge
- **Total Transfers:** 4
- **Total Volume (Celestia → Eden):** 1.6 TIA
- **Total Volume (Eden → Celestia):** 0.1 TIA
- **Net Balance on Eden:** 1.5 TIA
- **Gas Costs (Deployment):** 0.00464236003249652 ETH (Eden side)

---

## Notes

- All transfers are processed through the Hyperlane relayer infrastructure
- Relayer automatically picks up messages from both chains and delivers them
- Typical message delivery time: 1-5 minutes
- Both bridges are bidirectional and fully operational
- Router enrollments are permanent and don't need to be repeated

---

**Last Updated:** October 22, 2025
