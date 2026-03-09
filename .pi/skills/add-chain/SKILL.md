---
name: add-chain
description: Use this skill when adding a new chain registry entry under chains/. It runs an interactive walkthrough to collect chain metadata, then writes chains/<chain>/metadata.yaml and updates chains/metadata.yaml.
---

# Add Chain Skill

## When to use
Use this skill when the user wants to add a new chain to this repo's registry under `chains/`.

This skill is for metadata onboarding:
- create `chains/<chain>/metadata.yaml`
- update aggregate `chains/metadata.yaml`

## Source of truth
Always reference local files before writing:
- `chains/schema.json`
- `chains/celestiatestnet/metadata.yaml` (cosmosnative example)
- `chains/edentestnet/metadata.yaml` (ethereum example)
- `chains/metadata.yaml` (aggregate format/order)

Do not rely on memory for names, IDs, protocol values, or field shapes.

## Interaction model (mandatory)
Run this as an interactive walkthrough. Ask for missing values before writing any file.

Rules:
1. Ask one focused question at a time.
2. Echo back collected values in a final summary.
3. Ask for explicit confirmation before writing files.
4. If required values are missing, stop and ask for them.

## Required intake checklist
Collect these values:
- `name` (lowercase alphanumeric, starts with a letter; used as directory key)
- `displayName`
- `chainId` (number or string)
- `domainId` (number)
- `protocol` (`ethereum`, `cosmosnative`, etc. per schema/repo conventions)
- `isTestnet` (`true`/`false`)
- `nativeToken.name`
- `nativeToken.symbol`
- `nativeToken.decimals`
- `rpcUrls` (at least one URL)
- `technicalStack`

Recommended intake:
- `blocks.confirmations`
- `blocks.estimateBlockTime`
- `blocks.reorgPeriod`

If `protocol == cosmosnative`, additionally require:
- `bech32Prefix`
- `grpcUrls` (at least one URL)
- `restUrls` (at least one URL)
- `nativeToken.denom`

Cosmosnative strongly recommended (prompt for these too):
- `canonicalAsset`
- `gasPrice.amount` and `gasPrice.denom`
- `slip44` (typically `118`)
- `contractAddressBytes` (commonly `32`)
- `transactionOverrides.gasPrice`

## Workflow
### 1) Preflight and examples
1. Read `chains/celestiatestnet/metadata.yaml` and `chains/edentestnet/metadata.yaml`.
2. Use those files to explain the expected shape for cosmosnative vs ethereum.
3. Read `chains/schema.json` for field validity.

### 2) Interactive intake
1. Ask for all required values.
2. For URLs, normalize to:
```yaml
rpcUrls:
  - http: https://...
```
and likewise for `grpcUrls` and `restUrls`.
3. For numeric-like fields, preserve user intent (`string` vs `number`) where schema allows both.

### 3) Draft metadata
Create `chains/<name>/metadata.yaml` with:
- header: `# yaml-language-server: $schema=../schema.json`
- fields ordered similarly to existing examples for readability
- no placeholder secrets

Template (ethereum-like):
```yaml
# yaml-language-server: $schema=../schema.json
chainId: <chainId>
displayName: <displayName>
domainId: <domainId>
isTestnet: <true|false>
name: <name>
nativeToken:
  decimals: <decimals>
  name: <tokenName>
  symbol: <tokenSymbol>
protocol: <protocol>
rpcUrls:
  - http: <rpcUrl1>
technicalStack: <technicalStack>
```

Template (cosmosnative-like minimum):
```yaml
# yaml-language-server: $schema=../schema.json
bech32Prefix: <prefix>
chainId: <chainId>
displayName: <displayName>
domainId: <domainId>
grpcUrls:
  - http: <grpcUrl1>
isTestnet: <true|false>
name: <name>
nativeToken:
  decimals: <decimals>
  denom: <denom>
  name: <tokenName>
  symbol: <tokenSymbol>
protocol: cosmosnative
restUrls:
  - http: <restUrl1>
rpcUrls:
  - http: <rpcUrl1>
technicalStack: <technicalStack>
```

### 4) Update aggregate registry
Add/update the chain entry in `chains/metadata.yaml` so it matches `chains/<name>/metadata.yaml`.

Keep aggregate ordering/style consistent with surrounding entries.

### 5) Validation
After writing:
1. Re-open both files and compare values line-by-line.
2. Ensure cosmosnative entries include required extra fields above.
3. Confirm no unrelated chains changed.

## Guardrails
- Never invent chain IDs, domain IDs, or RPC endpoints.
- Never commit secrets/private keys.
- Do not proceed to file writes without explicit confirmation of the collected values.
- If user requests full chain onboarding, note that addresses files may also be needed (`chains/<chain>/addresses.yaml` and `chains/addresses.yaml`) but keep this skill focused on metadata unless asked.
