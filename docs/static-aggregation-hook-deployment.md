# Static Aggregation Hook Deployment

This document sketches how a `StaticAggregationHook` is composed and deployed in Hyperlane, and how it can be used as a Mailbox `requiredHook` so its checks cannot be bypassed by a caller-supplied custom hook.

## Summary

A `StaticAggregationHook` does not store its child hooks in mutable contract storage.

Instead:

1. A `StaticAggregationHookFactory` deploys a `StaticAggregationHook` implementation contract.
2. The factory deploys a `MetaProxy` instance for a specific ordered list of hook addresses.
3. That ordered hook list is embedded as immutable proxy metadata.
4. `StaticAggregationHook.hooks()` reads the metadata back out and decodes it as `address[]`.

This means:

- the hook composition is fixed at deployment time
- there is no `setHooks(...)` function
- changing the composition requires deploying a new aggregation hook address

Primary references:

- [StaticAggregationHookFactory.sol](../solidity/dependencies/hyperlane-monorepo-1.0.0/solidity/contracts/hooks/aggregation/StaticAggregationHookFactory.sol)
- [StaticAggregationHook.sol](../solidity/dependencies/hyperlane-monorepo-1.0.0/solidity/contracts/hooks/aggregation/StaticAggregationHook.sol)
- [StaticAddressSetFactory.sol](../solidity/dependencies/hyperlane-monorepo-1.0.0/solidity/contracts/libs/StaticAddressSetFactory.sol)
- [MetaProxy.sol](../solidity/dependencies/hyperlane-monorepo-1.0.0/solidity/contracts/libs/MetaProxy.sol)
- [Mailbox.sol](../solidity/dependencies/hyperlane-monorepo-1.0.0/solidity/contracts/Mailbox.sol)

## How Composition Is Defined

The concrete composition is passed to the factory as an ordered array of hook addresses:

```solidity
address aggregationHook = staticAggregationHookFactory.deploy(
  new address[](3)
);
```

More concretely:

```solidity
address[] memory hooks = new address[](3);
hooks[0] = address(merkleTreeHook);
hooks[1] = address(pausableHook);
hooks[2] = address(rateLimitedHook);

address aggregationHook = staticAggregationHookFactory.deploy(hooks);
```

Under the hood:

- `StaticAggregationHookFactory` inherits `StaticAddressSetFactory`
- `StaticAddressSetFactory.deploy(address[] calldata _values)` forwards to the threshold-based base class
- the base class computes:
  - `_metadata = abi.encode(_values, _threshold)`
  - `_bytecode = MetaProxy.bytecode(implementation, _metadata)`
  - `_salt = keccak256(_metadata)`
- the factory then deploys the metaproxy instance with `CREATE2`

Relevant code:

- `StaticAggregationHookFactory._deployImplementation()` returns `new StaticAggregationHook()`
- `StaticAddressSetFactory.deploy(address[] calldata _values)` uses `_values.length` as the threshold
- `StaticThresholdAddressSetFactory._saltAndBytecode(...)` packs the metadata and metaproxy bytecode
- `MetaProxy.bytecode(...)` appends the metadata to the proxy bytecode

## How The Aggregation Hook Reads Its Members

The `StaticAggregationHook` contract itself has no constructor args and no setter for child hooks.

Its `hooks(...)` function does this:

```solidity
return abi.decode(MetaProxy.metadata(), (address[]));
```

That is the key behavior:

- the proxy metadata is read from calldata by `MetaProxy.metadata()`
- the aggregation hook decodes that metadata as the ordered address array of child hooks
- the hook executes child hooks in that same order during `postDispatch`

Operationally, order matters. For example:

1. `MerkleTreeHook`
2. `PausableHook`
3. `RateLimitedHook`

would run in that exact order.

## Why Use It As `requiredHook`

The Mailbox executes hooks in two stages:

1. `requiredHook`
2. selected hook, which is either:
   - the caller-supplied custom hook, or
   - the Mailbox `defaultHook` if no custom hook is supplied

The relevant behavior in `Mailbox.dispatch(...)` is:

```solidity
requiredHook.postDispatch{value: requiredValue}(metadata, message);
hook.postDispatch{value: msg.value - requiredValue}(metadata, message);
```

This makes `requiredHook` the correct enforcement point for controls that must never be bypassed.

If a user overrides the Mailbox `defaultHook`, they still cannot skip the `requiredHook`.

## Recommended Pattern For Non-Bypassable Warp Transfer Controls

If the goal is to make these controls mandatory for warp transfers:

- Merkle insertion
- pause switch
- transfer volume rate limit

then the intended shape is:

```text
Mailbox.requiredHook
  = StaticAggregationHook([
      MerkleTreeHook,
      PausableHook,
      RateLimitedHook
    ])
```

With that setup:

- `MerkleTreeHook` always runs
- `PausableHook` always runs
- `RateLimitedHook` always runs
- a caller may still provide a custom default hook, but cannot bypass the required path

## Sketch Deployment Sequence

This is the high-level deployment sequence for an EVM Mailbox using a required aggregation hook.

### 1. Deploy or identify the Mailbox

You need the target Mailbox address first.

The Mailbox owner must be able to set:

- `requiredHook`
- optionally `defaultHook`

### 2. Deploy the component hooks

Deploy the concrete subhooks you want the aggregation hook to call.

Example:

1. Deploy `MerkleTreeHook(mailbox)`
2. Deploy `PausableHook()`
3. Deploy `RateLimitedHook(mailbox, maxCapacity, sender)`

Notes:

- `RateLimitedHook` is token-message specific and is intended for warp-route token messages.
- `PausableHook` is `Ownable`, so decide who should control pause and unpause.
- `RateLimitedHook` inherits the `RateLimited` control surface, so decide who should own refill-rate changes.

### 3. Deploy the aggregation hook factory

Deploy `StaticAggregationHookFactory` if it does not already exist in your environment.

Its constructor deploys the shared `StaticAggregationHook` implementation once and stores that implementation address internally.

### 4. Deploy the concrete aggregation hook instance

Call the factory with the ordered list of subhook addresses:

```solidity
address[] memory hooks = new address[](3);
hooks[0] = address(merkleTreeHook);
hooks[1] = address(pausableHook);
hooks[2] = address(rateLimitedHook);

address requiredAggregationHook = staticAggregationHookFactory.deploy(hooks);
```

This returns the address of the concrete `MetaProxy` instance representing exactly that hook list.

Important properties:

- the address is deterministic for the same ordered list
- reordering the hooks gives a different deployed address
- changing one subhook address gives a different deployed address

### 5. Set the Mailbox `requiredHook`

As the Mailbox owner, set:

```solidity
mailbox.setRequiredHook(requiredAggregationHook);
```

After that, every Mailbox dispatch through that Mailbox executes the aggregation hook path first.

### 6. Optionally set a `defaultHook`

You may still set a Mailbox `defaultHook` for normal dispatch fee/payment behavior.

That `defaultHook` remains overrideable by callers that use the custom-hook dispatch path, but the `requiredHook` remains mandatory.

## Example Solidity Sketch

```solidity
MerkleTreeHook merkleTreeHook = new MerkleTreeHook(address(mailbox));
PausableHook pausableHook = new PausableHook();
RateLimitedHook rateLimitedHook =
    new RateLimitedHook(address(mailbox), maxCapacity, address(tokenRouter));

StaticAggregationHookFactory factory = new StaticAggregationHookFactory();

address[] memory hooks = new address[](3);
hooks[0] = address(merkleTreeHook);
hooks[1] = address(pausableHook);
hooks[2] = address(rateLimitedHook);

address requiredAggregationHook = factory.deploy(hooks);

mailbox.setRequiredHook(requiredAggregationHook);
```

## Verification Checklist

After deployment, verify:

1. `mailbox.requiredHook()` returns the aggregation hook address.
2. `StaticAggregationHook(requiredAggregationHook).hooks("")` returns the expected ordered list of child hooks.
3. `mailbox.quoteDispatch(...)` includes the required hook path.
4. A paused `PausableHook` causes dispatch to revert.
5. A transfer above the configured rate limit causes dispatch to revert.
6. A caller-supplied custom hook still cannot bypass the required aggregation hook.

## Caveats

- `requiredHook` applies to every dispatch through that Mailbox path, not just one route, unless routing or app-level separation is introduced elsewhere.
- `RateLimitedHook` expects token-message semantics and an authorized sender address. It is not a generic message limiter for arbitrary Hyperlane applications.
- `StaticAggregationHook` uses the same metadata blob for all subhooks in the aggregation path. The hooks in this document do not depend on custom metadata, but other hook combinations may.
- If any subhook reverts, the whole dispatch reverts.

## Practical Conclusion

To compose `MerkleTreeHook`, `PausableHook`, and `RateLimitedHook`, you do not configure the aggregation hook after deployment.

You compose it by:

1. deploying the subhooks
2. passing their addresses in order to `StaticAggregationHookFactory.deploy(address[])`
3. setting the resulting aggregation hook address as the Mailbox `requiredHook`
