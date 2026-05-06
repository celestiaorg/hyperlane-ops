# Issue: Add Origin IGP Sweep Mode

## Summary

`igp-oracle` currently reconciles only explicitly configured origin and remote pairs. This is useful for early testing, but it does not match the operational shape of real IGP deployments where an origin IGP may contain many destination gas configs.

Add a dry-run mode that starts from an origin chain IGP, discovers all remote domains configured on-chain for that IGP, resolves any matching domains that exist in the local registry, skips unknown domains, and reconciles every resolvable target.

Write mode should remain single-target and explicit at first.

## Motivation

The Celestia mainnet sample in `sample-configs.celestia.json` shows an IGP with 141 destination gas configs. This looks like an operator-managed generated config set, not a hand-authored list of application-specific routes.

For a practical scheduled dry-run, operators should be able to ask:

```text
For this origin IGP, evaluate every remote domain that is already configured on-chain and that this registry knows how to price.
```

That is more realistic than requiring every remote domain to be duplicated manually in `igp-oracle.example.yaml`.

## Current Behavior

The resolver requires each enabled target to specify either:

- `remoteChain`
- `remoteDomain`

If neither is present, target resolution fails.

The current flow is:

1. Parse config.
2. Resolve concrete origin and remote target from registry/config.
3. Select the origin chain adapter.
4. Read the single target's current on-chain IGP config.
5. Compute proposal, compare deltas, and write artifacts.

This means the tool cannot currently discover remotes from on-chain IGP state.

## Proposed Behavior

Allow a target with `originChain` and no `remoteChain` / `remoteDomain`.

Example:

```yaml
targets:
  - originChain: celestiatestnet
    enabled: true
    remoteSelection: configuredOnOriginIgp
    gasOverheadPolicy: preserveCurrent
    gas:
      source: rpc
      min: "1"
      max: "1000000000000"
    exchangeRate:
      min: "1"
      max: "1000000000000000"
    write:
      enabled: false
      method: celestia-grpc
      signerProfile: celestia-owner
```

Open naming question: this could be represented as an explicit `remoteSelection` field, or it could be inferred from missing `remoteChain` and `remoteDomain`. Prefer an explicit field so config intent is unambiguous.

Dry-run behavior:

1. Resolve origin chain metadata and IGP identifier.
2. Ask the origin adapter to list all destination gas configs currently registered on that IGP.
3. For each returned `remoteDomain`, try to find matching chain metadata in the local registry.
4. If metadata exists, build a normal `ReconciliationTarget` and reconcile it.
5. If metadata is missing, skip that remote domain and record a skipped entry in the artifact.
6. Complete the run successfully if at least origin discovery succeeded, even when some domains are skipped.

Write behavior:

- Sweep mode must be dry-run only initially.
- `--write` should require explicit `--remote-chain` or `--remote-domain`.
- Bulk write can be considered later after single-target writes are proven.

## Skipped Domains

Unknown remote domains should not fail the entire run.

They should be recorded in artifacts:

```json
{
  "originChain": "celestiatestnet",
  "remoteDomain": 1,
  "status": "skipped",
  "code": "missing_registry_metadata",
  "reason": "no local chain metadata for remote domain 1"
}
```

Rationale:

- Mainnet IGPs can contain many domains outside this repo's operational scope.
- Failing the whole run would make sweep mode noisy and fragile.
- Silently ignoring missing domains would hide registry coverage gaps.

## Adapter API Changes

Add a discovery method to the origin chain adapter boundary.

Candidate shape:

```rust
#[async_trait::async_trait]
pub trait ChainAdapter {
    async fn list_igp_destination_configs(
        &self,
        origin: &ChainMetadata,
        origin_addresses: &CoreAddresses,
    ) -> Result<Vec<ConfiguredRemoteDomain>>;

    async fn read_igp_config(
        &self,
        target: &ReconciliationTarget,
    ) -> Result<IgpConfigRead>;

    async fn plan_update(
        &self,
        target: &ReconciliationTarget,
        proposed: &ProposedIgpConfig,
    ) -> Result<TxPlan>;
}
```

Candidate model:

```rust
pub struct ConfiguredRemoteDomain {
    pub remote_domain: u32,
    pub current: CurrentIgpConfig,
    pub source: OnChainReadSource,
}
```

The reconciliation engine can reuse `current` from discovery rather than re-reading each individual target when the adapter returns complete config data.

## Resolver Refactor

The current resolver is too early-bound to concrete remote targets. It should be split into two phases:

1. Resolve origin work items from config and CLI filters.
2. Expand each origin work item into concrete reconciliation targets.

Suggested types:

```rust
pub enum RemoteSelection {
    Chain(String),
    Domain(u32),
    ConfiguredOnOriginIgp,
}

pub struct OriginWorkItem {
    pub origin: ChainMetadata,
    pub origin_addresses: CoreAddresses,
    pub config: TargetConfig,
    pub selection: RemoteSelection,
}

pub enum ExpandedTarget {
    Reconcile {
        target: ReconciliationTarget,
        current: Option<IgpConfigRead>,
    },
    Skipped {
        origin_chain: String,
        remote_domain: u32,
        code: String,
        reason: String,
    },
}
```

The engine should then process `ExpandedTarget::Reconcile` through the existing proposal and policy path.

## Config Semantics

Sweep mode raises one important policy question: where does `gasOverhead` come from?

Options:

1. `preserveCurrent`: keep the on-chain overhead unless explicitly overridden.
2. `configuredDefault`: use a configured default overhead for every discovered remote.
3. `perDomainOverrides`: preserve current unless a domain-specific override exists.

Recommendation for the first implementation:

- Use `preserveCurrent` by default in sweep mode.
- Allow optional per-domain overrides later.
- Keep explicit pair mode as-is, where `gasOverhead` is required policy in config.

Reasoning:

- The Celestia mainnet sample shows common defaults but also exceptions.
- Sweep mode should avoid accidentally overwriting domain-specific overhead values.
- The first valuable sweep is price/gas oracle reconciliation, not overhead normalization.

## Artifact Changes

Artifacts should distinguish:

- resolved targets
- skipped remote domains
- discovery source

`igp-plan.json` candidate additions:

```json
{
  "targets": [],
  "skippedTargets": [
    {
      "originChain": "celestiatestnet",
      "remoteDomain": 1,
      "status": "skipped",
      "code": "missing_registry_metadata",
      "reason": "no local chain metadata for remote domain 1"
    }
  ],
  "discovery": [
    {
      "originChain": "celestiatestnet",
      "igpIdentifier": "0x...",
      "protocol": "cosmosnative",
      "configuredRemoteDomains": 141,
      "resolvedRemoteDomains": 12,
      "skippedRemoteDomains": 129
    }
  ]
}
```

Markdown summary should include:

- number of discovered domains
- number reconciled
- number skipped
- top update recommendations / policy violations
- skipped domain count, not every skipped domain by default

Full skipped-domain details belong in JSON.

## Exit Codes

Recommended behavior:

- `0`: sweep completed, no updates recommended, skipped domains are informational only.
- `10`: sweep completed and at least one resolved target needs an update.
- `20`: origin discovery failed due missing registry data or read endpoint.
- `30`: policy violation for at least one resolved target.

Skipped unknown domains should not force a non-zero exit by themselves.

## Protocol Support

### Cosmosnative

Cosmosnative discovery should use the protobuf/gRPC query for all destination gas configs on an IGP.

It must not shell out to `celestia-appd` from the Rust code.

### EVM

EVM discovery is harder because not every deployed contract exposes enumerable remote domains.

Initial EVM support can remain explicit-target only unless the adapter can discover domains from:

- indexed events over a configured block range
- an operator-provided domain list
- a known enumerable contract shape

Do not block cosmosnative sweep mode on EVM discovery.

## Testing Plan

Unit tests:

- config parses explicit remote target.
- config parses sweep target.
- resolver returns `OriginWorkItem` for sweep mode.
- expansion skips unknown domains and records `missing_registry_metadata`.
- expansion resolves known domains into `ReconciliationTarget`.
- sweep mode preserves current gas overhead by default.
- write mode rejects sweep targets.

Fixture tests:

- use `sample-configs.celestia.json` to simulate 141 configured domains.
- include at least one known local domain and several unknown domains.
- assert skipped-domain artifact shape is stable.
- assert resolved target uses decimal-aware token exchange-rate computation.

Adapter tests:

- cosmosnative list query decodes multiple destination gas configs.
- EVM adapter returns unsupported discovery unless an explicit discovery method exists.

## Suggested Implementation Phases

### Phase A: Spec and Config Shape

- Add `remoteSelection` and sweep-mode semantics to `SPEC.md`.
- Add config enum parsing.
- Keep existing explicit target config backward-compatible.

### Phase B: Resolver Split

- Introduce origin work item resolution.
- Keep explicit target resolution behavior passing existing tests.
- Add skipped target artifact model.

### Phase C: Cosmosnative Discovery

- Add adapter method for listing destination gas configs.
- Use existing protobuf layout and query client structure.
- Add fixture-backed tests using `sample-configs.celestia.json`.

### Phase D: Sweep Reconciliation

- Expand discovered configs into concrete targets.
- Preserve current `gasOverhead` by default.
- Reuse current config read from discovery where possible.
- Write updated JSON and Markdown artifacts.

### Phase E: CLI and Safety

- Allow scheduled dry-run sweep mode.
- Reject `--write` with sweep mode.
- Add targeted CLI filters for narrowing sweep output if needed.

## Open Questions

- Should sweep mode be inferred from missing `remoteChain` / `remoteDomain`, or require explicit `remoteSelection: configuredOnOriginIgp`?
- Should skipped domains be included in Markdown summary, or only counted there with details in JSON?
- Should unknown domains optionally fail the run in strict mode?
- Do we want per-domain gas overhead overrides in v1, or preserve-current only?
- Should market data mapping be required only for resolved domains, or prevalidated for all registry chains?
- Should EVM discovery be event-based later, or remain explicit-target only?

## Acceptance Criteria

- A config can declare one Celestia origin sweep target.
- Dry-run discovers all configured remote domains on that origin IGP.
- Domains missing from local registry are skipped and recorded.
- Domains present in local registry are reconciled with existing policy logic.
- Sweep dry-run does not require signer credentials.
- Sweep write mode is rejected unless an explicit remote target is selected.
- Artifacts clearly show discovered, reconciled, skipped, update-needed, and policy-violation counts.
