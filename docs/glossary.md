# Glossary

A quick reference for common Hyperlane terms used throughout this repo and documentation.

- **Agent**: An off-chain service that interacts with Hyperlane core contracts (e.g., relayers, validators).
- **Checkpoint**: A snapshot of the Merkle tree (root + index) used by validators to sign message sets.
- **Collateral Token**: A token locked on the origin chain to back synthetic representations on remote chains.
- **Core**: The base Hyperlane contracts/modules (Mailbox, ISM, hooks) required for messaging.
- **Domain**: A numeric identifier for a chain in Hyperlane’s registry (EVM `chainId` or Cosmos domain ID).
- **DomainRoutingISM**: An ISM that selects a per-domain verification module based on the message’s origin domain, allowing different security models for different chains.
- **Hook / Post-Dispatch Hook**: A module invoked by the Mailbox after dispatch to handle tasks like fee payment or Merkle tree insertion.
- **IGP (Interchain Gas Paymaster)**: A post-dispatch hook that collects gas fees for relaying messages to destination chains.
- **ISM (Interchain Security Module)**: The verification logic on the destination chain that determines if a message is valid.
- **Mailbox**: The core contract/module responsible for dispatching and processing interchain messages.
- **Merkle Tree Hook**: The post-dispatch hook that inserts message IDs into an incremental Merkle tree for checkpointing.
- **Metadata**: The proof data supplied to the ISM by relayers (e.g., validator signatures).
- **Registry**: The canonical set of chain metadata and contract addresses used by Hyperlane tooling (in this repo, the entries under `chains/`, `configs/` and `deployments/`).
- **Relayer**: An off-chain agent that submits messages and metadata to destination Mailboxes.
- **Routing ISM**: An ISM that selects a security module based on the origin domain.
- **Synthetic Token**: A token minted on a destination chain that represents collateral locked on the origin chain.
- **TokenRouter**: The Warp Route contract that dispatches and handles token transfer messages through the Mailbox.
- **Validator**: An off-chain agent that signs checkpoints for an origin chain and publishes signatures to storage.
- **ValidatorAnnounce**: The contract/module where validators publish their signature storage location.
- **Warp Route**: A Hyperlane token-bridging configuration connecting origin and destination chains for a specific asset.
