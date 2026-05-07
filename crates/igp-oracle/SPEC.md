# IGP Gas Oracle Workflow Specification

Status: Draft v0.1

Last updated: 2026-05-05

## Summary

This document specifies a workflow-based IGP config updater for Hyperlane chains maintained in this repository.

A one-shot CLI program performs reconciliation, and GitHub Actions provides scheduling, manual dispatch, approvals, secrets, artifacts, and notifications.

The default operating model is:

1. A scheduled GitHub Action runs the updater in `dry-run` mode.
2. The updater evaluates on-chain IGP config against current pricing and gas data.
3. If configured thresholds are exceeded, the workflow sends a Slack or IM notification with an audit summary.
4. A human operator reviews the summary.
5. The operator runs a protected manual `workflow_dispatch` job in `write` mode, or applies the generated transaction outside GitHub Actions.

The CLI is the core product. GitHub Actions is only the execution environment.

## Goals

- Keep IGP pricing updates inexpensive to operate.
- Avoid continuously running infrastructure.
- Make dry-run reconciliation cheap, repeatable, and safe.
- Put write mode behind explicit human approval.
- Use this repository as the source of truth for chain metadata and deployed core addresses.
- Update only IGP and gas oracle configuration, not broader core deployment state.
- Produce clear audit artifacts for every dry-run and write attempt.

## Non-Goals

- Automatically deploying or replacing Hyperlane hooks.
- Applying broad `hyperlane core apply` diffs as a recurring bot operation.
- Managing ISM routing, mailbox config, ownership transfers, or proxy admin config.
- Estimating application-specific destination gas for arbitrary messages.
- Replacing relayers or deciding relayer profitability.

## Operating Model

### Scheduled Dry Run

A cron-based GitHub Action runs the CLI in dry-run mode. This job:

- loads configured reconciliation targets
- reads current on-chain IGP config
- fetches gas and market data
- computes proposed values
- applies policy thresholds
- writes a Markdown summary and machine-readable JSON artifact
- sends a notification only when action is needed or when the job fails

`igp-oracle` does not send Slack, IM, or webhook notifications directly. It emits artifacts, logs, and exit codes. GitHub Actions owns notification delivery.

Dry-run must not require write credentials. It may require read RPC credentials and market data API keys.

### Manual Write

A separate `workflow_dispatch` path runs the same CLI in write mode. This job:

- requires explicit workflow inputs
- uses GitHub environment protection
- recomputes all target values from fresh data
- rejects stale dry-run artifacts unless the operator explicitly allows them as reference-only context
- prints the exact planned transaction targets and calldata or command arguments
- submits only allowlisted IGP or gas oracle update transactions
- reads back on-chain state after confirmation
- writes a final audit artifact

### External Write

The dry-run artifact should also be useful outside GitHub Actions. For example, operators may use the proposed transaction data with:

- a local CLI
- a multisig workflow
- a Safe module
- a cloud KMS signer
- `celestia-appd --generate-only`

The GitHub workflow must not be the only path to apply a reviewed update.

## Source of Truth

The updater reads repository files first:

- `chains/<chain>/metadata.yaml`
  - `name`
  - `domainId`
  - `chainId`
  - `protocol`
  - `rpcUrls`
  - `nativeToken.symbol`
  - `nativeToken.decimals`
  - `gasPrice` where present
- `chains/<chain>/addresses.yaml`
  - `interchainGasPaymaster`
  - `mailbox`
  - related core addresses for validation
- `chains/metadata.yaml`
- `chains/addresses.yaml`
- updater-specific target config

The following inputs are operator policy and should be explicit in updater config:

- enabled origin and remote pairs
- gas overhead per target
- token market data asset IDs
- min and max update thresholds
- absolute clamps for gas price and exchange rate
- cooldown policy
- signer profile per writable origin chain
- write method per protocol

## Reconciliation Values

For each origin chain and remote domain, the updater evaluates:

- `gasPrice`: remote chain gas price in remote native units
- `tokenExchangeRate`: remote native token quoted in origin native token, scaled by `1e10`
- `gasOverhead`: current on-chain overhead for the remote domain, preserved by sweep mode unless a future override policy changes it

`tokenExchangeRate` should remain market-derived and meaningful. If the final
quote needs operational padding, prefer adjusting gas policy or overhead rather
than distorting the token exchange rate.

The core pricing formula is:

```text
tokenExchangeRate =
  remoteNativePriceUsd
  / originNativePriceUsd
  * 10^(originNativeTokenDecimals - remoteNativeTokenDecimals)
  * 1e10
```

Then the updater applies configured safety margins, rounding rules, and clamps.

The native token decimal adjustment is required. The IGP quote is charged in
the origin chain's smallest native unit, while `gasPrice` is denominated in the
remote chain's smallest native unit. For example, Celestia uses `utia` with 6
decimals and Ethereum uses `wei` with 18 decimals. A Celestia-origin,
Ethereum-remote exchange rate therefore includes a `10^(6 - 18)` factor before
the Hyperlane `1e10` exchange-rate scale is applied.

`gasOverhead` is not market data. Sweep mode preserves the current on-chain value in v1 so the updater does not accidentally normalize domain-specific overhead exceptions.

### On-Chain IGP Semantics

The updater must model stored IGP config values, not just quote helper outputs.
Both EVM and cosmosnative IGP implementations use the same economic equation:

```text
payment =
  (applicationGasLimit + gasOverhead)
  * gasPrice
  * tokenExchangeRate
  / 1e10
```

The resulting payment is denominated in the origin IGP's payment denom or native
token smallest unit.

Implementation notes from the EVM IGP:

- `TOKEN_EXCHANGE_RATE_SCALE` is `1e10`.
- `gasPrice` is the remote chain gas price in the remote native token's smallest unit.
- `tokenExchangeRate` converts remote native units into origin native units using the `1e10` scale.
- `destinationGasLimit(remoteDomain, gasLimit)` returns `gasOverhead + gasLimit`.
- Current Hyperlane EVM `main` stores native-token gas oracles in `tokenGasOracles[NATIVE_TOKEN][remoteDomain]` and overhead in `destinationGasOverhead[remoteDomain]`; the compatibility getter `destinationGasConfigs(remoteDomain)` returns the native gas oracle and overhead.
- Older deployed contracts may store both fields in the public `destinationGasConfigs` mapping. The EVM adapter must support the deployed ABI shape used by the target chain.
- Public `quoteGasPayment` should not be used as the only full-dispatch verification method unless the caller has already included overhead in the supplied gas amount. Prefer `destinationGasLimit(...)` plus `getExchangeRateAndGasPrice(...)`, or the mailbox/hook quote path that applies overhead.

Implementation notes from the cosmosnative IGP:

- `QuoteGasPayment` loads `DestinationGasConfig` for `(igpId, destinationDomain)`.
- It adds `GasOverhead` to the supplied gas limit before computing the payment.
- It computes `destinationCost = gasLimitWithOverhead * GasOracle.GasPrice`.
- It computes `amount = destinationCost * GasOracle.TokenExchangeRate / TokenExchangeRateScale`.
- The resulting coin denom is the denom configured when the IGP was created.
- `SetDestinationGasConfig` is owner-gated and stores `RemoteDomain`, `GasOracle`, and `GasOverhead`; it rejects missing gas oracle data.

The reconciliation engine must therefore compare and propose the three stored
config values directly:

- `gasPrice`
- `tokenExchangeRate`
- `gasOverhead`

It must not infer correctness from a single quote result unless the quote path's
overhead behavior is known for that protocol adapter.

### Celestia Mainnet Reference Configs

`sample-configs.celestia.json` is a useful behavioral reference from a
Celestia mainnet IGP query. It lives next to this spec in the `igp-oracle`
crate:

```bash
celestia-appd q hyperlane hooks destination-gas-configs \
  0x726f757465725f706f73745f6469737061746368000000040000000000000001 \
  --node https://celestia-rpc.publicnode.com:443 -o json
```

Observed characteristics:

- The queried IGP has 141 destination gas configs.
- `remote_domain: 1` corresponds to Ethereum mainnet and is configured as:
  - `token_exchange_rate: "101"`
  - `gas_price: "300000000"`
  - `gas_overhead: "174289"`
- `117 / 141` entries use `gas_overhead: "174289"`, suggesting that overhead is often a broad operator policy default rather than a measured per-domain value.
- `token_exchange_rate` values cluster around small integers such as `1`, `30`, and `101` for many EVM domains. This is expected once origin and remote native-token decimals are included in the exchange-rate calculation.
- A Celestia-origin, Ethereum-remote value around `101` is plausible with current ETH/TIA market prices and a conservative safety buffer. Without the decimal adjustment, the computed exchange rate would be off by orders of magnitude.

Use these mainnet values as fixtures and sanity checks for formula behavior, not
as authoritative target values for every deployment. The owning operator appears
to manage a broad, generated config set across many domains, including domains
that may not be actively connected for a particular application.

## CLI Program

Recommended binary name:

```text
igp-oracle
```

Operator-facing workflow name:

```text
IGP Gas Oracle
```

### Commands

The CLI should support one main reconciliation command:

```bash
igp-oracle reconcile --config crates/igp-oracle/igp-oracle.example.yaml --dry-run
igp-oracle reconcile --config crates/igp-oracle/igp-oracle.example.yaml --origin celestiatestnet --remote-chain edentestnet --dry-run
igp-oracle reconcile --config crates/igp-oracle/igp-oracle.example.yaml --origin celestiatestnet --remote-domain 2147483647 --write
```

When `--origin` is provided without `--remote-chain` or `--remote-domain`,
the command runs in origin IGP sweep mode. Sweep mode queries the origin IGP for
all configured destination gas configs, resolves only domains present in the
local registry, and records skipped domains in the artifacts.

Sweep mode must skip `remoteDomain == origin.domainId` with code
`self_domain`. An origin chain's own domain is not a real cross-chain fee path
and should not affect update decisions. Operators can still inspect this entry
explicitly by passing `--remote-chain <origin>` or `--remote-domain
<origin-domain-id>`.

Required flags:

- `--config <path>`
- `--registry <path>`, default `.`
- `--dry-run`, default behavior
- `--write`
- `--origin <chain>`
- `--remote-chain <chain>`
- `--remote-domain <domainId>`
- `--output-dir <path>`
- `--format markdown,json`

Useful write-mode flags:

- `--require-fresh-data`
- `--max-bps-change <bps>`
- `--allow-large-change`
- `--generate-only`
- `--confirm-tx-targets`

`--write` must be mutually exclusive with implicit defaults. Operators should have to ask for writes explicitly.

### Exit Codes

Recommended exit behavior:

- `0`: reconciliation completed and no write is needed, or write succeeded
- `10`: dry-run completed and update is recommended
- `20`: stale or missing market data
- `21`: stale or missing gas data
- `30`: policy violation or unsafe delta
- `40`: signer unavailable or unauthorized
- `50`: write submitted but verification failed

The GitHub Action can use these codes to decide whether to notify operators without treating every actionable dry-run as a failed CI job.

## Configuration

Example:

```yaml
marketData:
  provider: coingecko
  cacheTtlSeconds: 60
  staleAfterSeconds: 300
  assets:
    celestiatestnet: celestia
    sepolia: ethereum
    edentestnet: ethereum

defaults:
  minBpsChangeToWrite: 500
  maxBpsChangePerUpdate: 5000
  cooldownSeconds: 900
  safetyMultiplierBps: 11000
  gasSampleFreshnessSeconds: 120

targets:
  - originChain: celestiatestnet
    remoteSelection: configuredOnOriginIgp
    enabled: true
    gas:
      source: rpc
      min: "1"
      max: "1000000000000"
    exchangeRate:
      min: "1"
      max: "1000000000000000"
    write:
      enabled: true
      method: celestia-grpc
      signerProfile: celestia-owner

signers:
  celestia-owner:
    protocol: cosmosnative
    from: owner
    keyEnv: HYP_KEY_COSMOSNATIVE
```

The config should be versioned in this repo, but secrets must only be read from environment variables or external secret managers.

`remoteSelection: configuredOnOriginIgp` is the only supported configured
operating mode. It means the origin IGP's on-chain destination gas config list
is the source of truth for which remote domains exist. CLI flags such as
`--remote-chain` and `--remote-domain` are runtime filters over that sweep; they
narrow which resolved domains are evaluated for gas and market data during a
single invocation.

## Data Sources

### Gas

For remote EVM chains:

- use RPC gas price or fee history
- prefer a median over multiple samples
- apply a configured safety multiplier
- apply per-chain floor and cap

For remote cosmosnative chains:

- start with `chains/<chain>/metadata.yaml` gas price when fixed or policy-based
- preserve the raw metadata value in artifacts
- round fractional values up because on-chain IGP `gasPrice` is an integer
- include the rounding mode and reason in artifacts
- add direct chain query support later if needed

### Token Prices

For token prices:

- use CoinGecko or a configured market data provider
- batch all configured CoinGecko asset IDs into one request per cache window
- require fresh timestamps
- reject missing, stale, zero, or outlier values
- cache values only for the current run
- require explicit asset ID mapping where symbols are ambiguous

## Chain Adapters

After registry parsing, all protocol-specific behavior must go through chain adapters. The reconciliation engine should not branch directly on `protocol: ethereum` or `protocol: cosmosnative` except when selecting an adapter.

The registry loader should produce a normalized `ChainMetadata` model with fields such as:

- chain name
- domain ID
- chain ID
- protocol
- RPC endpoints
- native token symbol and decimals
- deployed IGP identifier where known

The target resolver should then create `ReconciliationTarget` records using normalized origin and remote metadata. From that point onward, protocol-specific reads, transaction planning, transaction submission, and verification belong to the selected adapter.

Recommended Rust trait shape:

```rust
#[async_trait::async_trait]
pub trait ChainAdapter {
    fn protocol(&self) -> ChainProtocol;

    async fn read_igp_config(
        &self,
        target: &ReconciliationTarget,
    ) -> Result<CurrentIgpConfig>;

    async fn plan_update(
        &self,
        target: &ReconciliationTarget,
        proposed: &ProposedIgpConfig,
    ) -> Result<TxPlan>;

    async fn submit_update(
        &self,
        target: &ReconciliationTarget,
        plan: &TxPlan,
    ) -> Result<TxReceipt>;

    async fn verify_update(
        &self,
        target: &ReconciliationTarget,
        expected: &ProposedIgpConfig,
    ) -> Result<VerificationResult>;
}
```

Adapter boundaries:

- `RegistryLoader` parses repository files only.
- `TargetResolver` maps operator intent to concrete origin and remote targets.
- `GasAdapter` fetches remote gas prices and may also be protocol-specific.
- `ChainAdapter` reads current IGP config and plans or submits origin-chain transactions.
- `ReconciliationEngine` compares current and proposed values without knowing whether the origin is EVM or cosmosnative.

This separation is required from the first implementation phase so that Celestia/cosmosnative support does not leak into EVM logic, and EVM contract assumptions do not leak into Celestia transaction generation.

### Celestia / Cosmosnative Origin

Celestia is the preferred v1 write target because this repo already documents the narrow update command:

```bash
celestia-appd tx hyperlane hooks igp set-destination-gas-config \
  [igp-id] [remote-domain] [token-exchange-rate] [gas-price] [gas-overhead] \
  [flags]
```

Dry-run output should include the exact command arguments and, where supported, a `--generate-only` transaction payload.

The Rust implementation must not shell out to `celestia-appd` for live reads.
Cosmosnative reads and transaction planning should use protobuf/gRPC clients
generated or modeled from the Hyperlane Cosmos module definitions. CLI command
forms may still be emitted as operator-facing reference artifacts for external
manual execution.

Write mode requirements:

- resolve the IGP ID from `chains/<origin>/addresses.yaml` or explicit config
- require the signer to be authorized for that IGP
- submit only the destination gas config update
- read back the destination gas config after submission
- verify all three values match the target

### EVM Origin

EVM support is viable but should be stricter than Celestia in v1.

The updater must not assume that a chain has a writable IGP just because a mailbox exists. It must resolve:

- IGP address
- remote domain
- gas oracle address for that remote domain
- IGP owner or authorized operator
- gas oracle owner or authorized operator

Write mode may only call allowlisted selectors such as:

- `StorageGasOracle.setRemoteGasDataConfigs`
- `InterchainGasPaymaster.setDestinationGasConfigs`

If the current default hook is `protocolFee`, if the IGP is nested in an aggregation or routing hook that cannot be safely resolved, or if ownership cannot be validated, the updater must fail closed.

The updater must not deploy a replacement hook or update mailbox hook config.

## GitHub Actions Design

### Scheduled Dry Run Workflow

Trigger:

```yaml
on:
  schedule:
    - cron: "*/30 * * * *"
  workflow_dispatch:
    inputs:
      origin:
        required: false
      remote:
        required: false
```

Behavior:

1. Check out the repository.
2. Install or download `igp-oracle`.
3. Run `igp-oracle reconcile --dry-run`.
4. Upload Markdown and JSON artifacts.
5. Add the Markdown summary to the GitHub Actions step summary.
6. Send Slack or IM notification when:
   - an update is recommended
   - data is stale
   - a policy violation occurs
   - the job fails unexpectedly

This job should not have signer secrets.

### Manual Write Workflow

Trigger:

```yaml
on:
  workflow_dispatch:
    inputs:
      origin:
        required: true
      remote:
        required: true
      max_bps_change:
        required: true
      generate_only:
        required: false
      write:
        required: true
```

Behavior:

1. Require a protected GitHub environment.
2. Check out the repository.
3. Install or download `igp-oracle`.
4. Recompute current target values.
5. Print the transaction plan.
6. Enforce target address and selector allowlists.
7. Submit or generate the transaction.
8. Wait for confirmation if submitted.
9. Re-read on-chain config.
10. Upload final audit artifacts.
11. Notify Slack or IM with success, failure, or generated transaction details.

Write mode should be scoped to one origin and one remote target at first. Batch writes can be added after single-target operations are proven.

## Artifact Format

Each run should produce:

```text
artifacts/
  igp-summary.md
  igp-plan.json
```

`igp-plan.json` is the canonical machine-readable artifact. It should include:

- run ID
- git commit SHA
- run-level policy thresholds
- discovery and skipped-domain results
- origin chain
- remote chain
- remote domain
- current on-chain values
- computed target values
- gas source, sampled gas price, and gas endpoint/provenance
- raw gas amount/denom, sampled integer gas price, final proposed gas price, and rounding policy where applicable
- price provider, market asset IDs, sampled prices, native token decimals, and decimal adjustment
- deltas in basis points
- policy decision with observed delta, write threshold, and max allowed delta as separate fields
- data source timestamps
- proposed transaction target
- proposed calldata or command arguments
- transaction planning error details when reconciliation succeeds but tx payload construction fails

Transaction plan data should live on each target in `igp-plan.json`. A separate
`tx-plan.json` should not be emitted by default because it duplicates a filtered
view of the canonical plan. If a later workflow needs a transaction-only file,
make it an explicit opt-in output.

Tx planning failures must not erase successful dry-run evaluation data. If a
target has current values, proposal inputs, proposed values, deltas, and an
`update_recommended` or `policy_violation` decision, those fields should remain
in the artifact and the tx failure should be recorded in `txPlanError`.

Target-level errors should use specific decision statuses so workflow
notifications can route them accurately:

- `config_error`: invalid operator config or target setup
- `market_data_error`: missing, stale, rate-limited, or invalid market prices
- `gas_data_error`: remote gas source failure or invalid gas sample
- `onchain_read_error`: origin IGP/gRPC/read-plan source failure
- `policy_error`: local policy calculation failure
- `data_source_error`: fallback for uncategorized data source failures

The Markdown summary should be concise enough to paste into Slack.

## Safety Controls

The updater must include:

- dry-run as the default behavior
- explicit `--write`
- per-target enable flags
- per-target write enable flags
- min delta before write
- max delta per write
- absolute min and max clamps
- stale data rejection
- zero-value rejection
- signer authorization checks where possible
- transaction target allowlist
- calldata selector allowlist for EVM writes
- single-target writes for the initial release

Recommended binary defaults:

- write only when change is at least 5 percent
- abort when change exceeds 50 percent unless explicitly overridden
- reject market data older than 5 minutes
- reject gas samples older than 2 minutes

Recommended workflow notification policy:

- notify when either `gasPrice` or `tokenExchangeRate` changes by at least 5 percent
- notify when `igp-oracle` exits with stale-data, policy-violation, write-failed, or unexpected failure status

## Secrets

Dry-run jobs should not use write credentials.

Write jobs may use one of:

- GitHub Actions environment secret containing a constrained hot key
- GitHub OIDC to cloud KMS
- OpenZeppelin Defender relayer
- Safe module or multisig generation flow

If a hot key is used, it should:

- own only the IGP or gas oracle contracts needed for updates
- have limited native token balance
- not own mailbox, ISM, proxy admin, treasury, or unrelated contracts
- be rotated periodically

## Notifications

Slack or IM notifications should be sent for:

- update recommended in dry-run
- policy violation
- stale data
- write generated
- write applied
- write failed

The notification should include:

- origin and remote target
- current values
- proposed values
- delta percentages
- policy decision
- GitHub run URL
- artifact names
- transaction hash when applicable

The updater should not spam when no action is needed. A no-op scheduled dry-run can remain visible only in GitHub Actions history.

## Implementation Language

`igp-oracle` will be implemented in Rust.

Reasons:

- good fit for a single-purpose, strongly typed ops binary
- reliable integer handling for on-chain values
- excellent CLI and serialization libraries
- easy static-ish distribution through GitHub Actions artifacts or releases
- strong failure handling with explicit result types
- aligns well with safety-critical transaction planning

Recommended Rust stack:

- `clap` for CLI parsing
- `serde`, `serde_yaml`, and `serde_json` for config and artifacts
- `reqwest` for HTTP
- `tokio` for concurrent data fetches
- `alloy` or `ethers-rs` for EVM reads and calldata generation
- `tonic` and `prost` for cosmosnative protobuf/gRPC reads
- `cosmrs` or protobuf transaction builders for cosmosnative write payloads
- `tracing` for structured logs
- `thiserror` or `anyhow` for error handling

The CLI must be deterministic, strongly validate integer values and policy bounds, and emit reviewable transaction plans before writing.

## Implementation Phases

### Phase 0: Workflow Spec and Config

- finalize target config schema
- confirm v1 origin and remote chains
- confirm signer model
- scaffold Rust crate
- define normalized metadata and chain adapter traits
- define GitHub workflow inputs and environment protection

### Phase 1: Dry-Run CLI

Deliver:

- registry loader
- target resolver
- chain adapter trait and cosmosnative adapter skeleton
- EVM adapter skeleton for read-only planning
- market data adapter
- gas data adapter
- policy engine
- Markdown and JSON artifacts

Exit criteria:

- scheduled dry-run can identify update recommendations without signer secrets

### Phase 2: Notification Workflow

Deliver:

- GitHub scheduled workflow
- GitHub step summary output
- Slack or IM webhook notification
- clean handling for no-op, update-needed, stale-data, and policy-violation outcomes

Exit criteria:

- operators are notified only when action is needed or the job fails

### Phase 3: Manual Celestia Write

Deliver:

- `--write` for one Celestia origin and one remote target
- `--generate-only` support where useful
- post-write readback and verification
- protected GitHub environment for write mode

Exit criteria:

- an operator can safely update one Celestia IGP destination gas config through workflow dispatch

### Phase 4: EVM Read Support

Deliver:

- IGP and gas oracle discovery
- ownership validation
- dry-run transaction planning for EVM targets

Exit criteria:

- operators can review proposed EVM IGP updates without writing

### Phase 5: EVM Write Support

Deliver:

- direct allowlisted setter calls
- post-write verification
- single-target workflow dispatch

Exit criteria:

- supported EVM-origin IGP configs can be updated without touching broader core config

## Open Questions

- Which origin chains are v1 targets?
- Should Celestia writes use `celestia-appd` directly, generated unsigned transactions, or both?
- Which Slack or IM channel should receive alerts?
- Should write mode require two consecutive dry-runs with similar values before allowing submission?
- Should GitHub Actions download a released binary, build from source, or use a checked-in toolchain lockfile?
- Which market data provider should be primary, and do we need a fallback for v1?
