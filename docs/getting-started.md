# Getting Started

## Hyperlane Architecture Overview
Hyperlane is a modular cross-chain messaging protocol. It is structured around on-chain components that produce and verify messages, plus off-chain agents that transport messages and attest to their correctness.

### Core on-chain components:

- The `Mailbox` is central in the Hyperlane protocl architecture, it is used for dispatching and processing interchain messages on each chain.
- The `Post-dispatch hooks` are contracts (or modules) invoked on message dispatch to handle tasks like fee payment (IGP hook) and merkle tree insertion (Merkle Tree Hook).
- The `ISM (Interchain Security Module)` is the verification layer on the destination chain; it decides the security model and ultimately whether a message can be processed based.

### Core off-chain services:

- For multisig bridges Validator agents index origin-chain messages, sign checkpoints (roots), and publish signatures to public storage.
- Relayers fetch messages and validator signatures, build ISM metadata, and submit `Mailbox.process()` on the destination chain.

### Message flow (high level):

- A message is dispatched to the origin `Mailbox`, which runs post-dispatch hooks (e.g., Merkle Tree Hook + IGP).
- Validators sign the latest checkpoint and publish signatures off-chain.
- A relayer gathers signatures, packages ISM metadata, and delivers the message to the destination `Mailbox`.
- The destination `Mailbox` calls the configured ISM to verify the message before executing it.

```
Origin Chain                                           Destination Chain
+---------------------+                                +---------------------+
|  Warp Token Router  |                                |  Warp Token Router  |
|  (Token Logic)      |                                |  (Token Logic)      |
+----------+----------+                                +----------+----------+
           | dispatch()                                           ^ handle()
           v                                                      |
+---------------------+      message + metadata        +---------------------+
|       Mailbox       |------------------------------->|       Mailbox       |
+----------+----------+                                +----------+----------+
           | post-dispatch hooks                                  |
           v                                                      | verify()
+---------------------+                                           v
| Post Dispatch Hooks |                                +----------+----------+
|  - IGP (fees)       |                                |         ISM         |
|  - Merkle Hook      |                                |  (Verification)     |
+---------------------+                                +---------------------+

```
