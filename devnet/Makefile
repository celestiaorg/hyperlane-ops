COMPOSE := docker compose -f docker-compose.yml

# Domains
ANVIL_DOMAIN ?= 1234
CELESTIADEV_DOMAIN ?= 69420

# Warp route identifiers
ANVIL_TIA_ROUTER ?= 0xa85233C63b9Ee964Add6F2cffe00Fd84eb32338f
CELESTIADEV_TIA_TOKEN_ID ?= 0x726f757465725f61707000000000000000000000000000010000000000000000

# Test accounts used by the devnet
ANVIL_DEFAULT ?= 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
ANVIL_DEFAULT_PK ?= 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80

CELESTIA_DEFAULT ?= celestia1d2qfkdk27r2x4y67ua5r2pj7ck5t8n4890x9wy
CELESTIA_DEFAULT_HEX ?= 0x6A809B36CAF0D46A935EE76835065EC5A8B3CEA7
CELESTIA_DEFAULT_RECIPIENT_B32 ?= 0x0000000000000000000000006a809b36caf0d46a935ee76835065ec5a8b3cea7

ANVIL_DEFAULT_RECIPIENT_B32 ?= 0x000000000000000000000000f39fd6e51aad88f6f4ce6ab8827279cfffb92266

# Transfer amount
AMOUNT_UNITS ?= 1000000

# Celestia tx tuning
CELESTIA_TX_GAS ?= 350000
CELESTIA_TX_FEES ?= 35000utia
CELESTIA_MAX_HYP_FEE ?= 40000utia

TXHASH ?=
ADDR ?=

.PHONY: start stop ps logs logs-relayer logs-validator-anvil logs-validator-celestiadev \
	require-validator-key query-celestia-warp-token query-celestia-warp-remotes \
	query-anvil-router quote-celestia-transfer quote-anvil-transfer balance-anvil \
	balance-celestia transfer-celestia query-celestia-tx \
	debug-celestia-addr transfer-anvil

start: require-validator-key
	@$(COMPOSE) up -d

stop:
	@$(COMPOSE) down -v

require-validator-key:
	@test -n "$(HYP_VALIDATOR_CHECKPOINT_KEY)" || \
		{ echo "HYP_VALIDATOR_CHECKPOINT_KEY must be set"; exit 1; }

ps:
	@$(COMPOSE) ps

logs:
	@$(COMPOSE) logs -f

logs-relayer:
	@docker logs -f relayer

logs-validator-anvil:
	@docker logs -f validator-anvil

logs-validator-celestiadev:
	@docker logs -f validator-celestiadev

query-celestia-warp-token:
	@$(COMPOSE) exec -T celestia-validator \
		celestia-appd query warp token \
		$(CELESTIADEV_TIA_TOKEN_ID) \
		--node http://localhost:26657 -o json | jq

query-celestia-warp-remotes:
	@$(COMPOSE) exec -T celestia-validator \
		celestia-appd query warp remote-routers \
		$(CELESTIADEV_TIA_TOKEN_ID) \
		--node http://localhost:26657 -o json | jq

query-anvil-router:
	@$(COMPOSE) exec -T anvil \
		cast call --rpc-url http://localhost:8545 \
		$(ANVIL_TIA_ROUTER) \
		"routers(uint32)(bytes32)" \
		$(CELESTIADEV_DOMAIN)

quote-celestia-transfer:
	@$(COMPOSE) exec -T celestia-validator \
		celestia-appd query warp quote-transfer \
		$(CELESTIADEV_TIA_TOKEN_ID) \
		$(ANVIL_DOMAIN) \
		--node http://localhost:26657 -o json | jq

quote-anvil-transfer:
	@$(COMPOSE) exec -T anvil \
		cast call --rpc-url http://localhost:8545 \
		$(ANVIL_TIA_ROUTER) \
		"quoteGasPayment(uint32)(uint256)" \
		$(CELESTIADEV_DOMAIN)

balance-anvil:
	@$(COMPOSE) exec -T anvil \
		cast call --rpc-url http://localhost:8545 \
		$(ANVIL_TIA_ROUTER) \
		"balanceOf(address)(uint256)" \
		$(ANVIL_DEFAULT)

balance-celestia:
	@$(COMPOSE) exec -T celestia-validator \
		celestia-appd query bank balances \
		$(CELESTIA_DEFAULT) \
		--node http://localhost:26657 -o json | jq

transfer-celestia:
	@$(COMPOSE) exec -T celestia-validator \
		celestia-appd tx warp transfer \
		$(CELESTIADEV_TIA_TOKEN_ID) \
		$(ANVIL_DOMAIN) \
		$(ANVIL_DEFAULT_RECIPIENT_B32) \
		$(AMOUNT_UNITS) \
		--from default \
		--chain-id celestiadev \
		--node http://localhost:26657 \
		--home /home/celestia/.celestia-app \
		--keyring-backend test \
		--gas $(CELESTIA_TX_GAS) \
		--fees $(CELESTIA_TX_FEES) \
		--max-hyperlane-fee $(CELESTIA_MAX_HYP_FEE) \
		--yes \
		-o json | jq

query-celestia-tx:
	@test -n "$(TXHASH)" || { echo "TXHASH must be set"; exit 1; }
	@$(COMPOSE) exec -T celestia-validator \
		celestia-appd query tx \
		$(TXHASH) \
		--node http://localhost:26657 -o json | jq

debug-celestia-addr:
	@test -n "$(ADDR)" || { echo "ADDR must be set"; exit 1; }
	@$(COMPOSE) exec -T celestia-validator \
		celestia-appd debug addr $(ADDR)

transfer-anvil:
	@$(COMPOSE) exec -T anvil sh -c 'HYP_FEE=$$(cast call --rpc-url http://localhost:8545 \
		$(ANVIL_TIA_ROUTER) \
		"quoteGasPayment(uint32)(uint256)" \
		$(CELESTIADEV_DOMAIN)); \
		cast send --rpc-url http://localhost:8545 \
		--private-key $(ANVIL_DEFAULT_PK) \
		--value "$$HYP_FEE" \
		$(ANVIL_TIA_ROUTER) \
		"transferRemote(uint32,bytes32,uint256)" \
		$(CELESTIADEV_DOMAIN) \
		$(CELESTIA_DEFAULT_RECIPIENT_B32) \
		$(AMOUNT_UNITS)'
