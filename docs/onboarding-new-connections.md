# Onboarding New Connections (Celestia Mocha)

To create a new connection (a prerequisite for warp route deployment), update the Hyperlane core deployment to support the remote chain's domain identifier. This is a two-step process:
1. Register the domain identifier with a gas config in the Interchain Gas Paymaster (IGP) destination gas configs.
2. Register the domain identifier in the Interchain Security Module (ISM) domain routing config.

## Add a New Domain to IGP Destination Gas Configs
This is required for sending messages to a remote (counterparty) chain.

```bash
celestia-appd tx hyperlane hooks igp set-destination-gas-config [igp-id] [remote-domain] [token-exchange-rate] [gas-price] [gas-overhead] --from owner --fees 800utia
```

## Add a New Domain to the Routing ISM
This is required for receiving messages from a remote (counterparty) chain.

```bash
celestia-appd tx hyperlane ism set-routing-ism-domain [routing-ism-id] [domain] [ism-id] --from owner --fees 800utia
```
