COMPOSE := docker compose -f docker-compose.yml

# Domains
ANVIL_DOMAIN=1234
CELESTIADEV_DOMAIN=69420

# Warp route identifiers
ANVIL_TIA_ROUTER=0xa85233C63b9Ee964Add6F2cffe00Fd84eb32338f
CELESTIADEV_TIA_TOKEN_ID=0x726f757465725f61707000000000000000000000000000010000000000000000

# Test accounts used by the devnet
ANVIL_DEFAULT=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
ANVIL_DEFAULT_PK=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80

CELESTIA_DEFAULT=celestia1d2qfkdk27r2x4y67ua5r2pj7ck5t8n4890x9wy
CELESTIA_DEFAULT_HEX=0x6A809B36CAF0D46A935EE76835065EC5A8B3CEA7
CELESTIA_DEFAULT_RECIPIENT_B32=0x0000000000000000000000006a809b36caf0d46a935ee76835065ec5a8b3cea7

ANVIL_DEFAULT_RECIPIENT_B32=0x000000000000000000000000f39fd6e51aad88f6f4ce6ab8827279cfffb92266

# Transfer amount
AMOUNT_UNITS=1000000 # 1 TIA, 6 decimals

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

.PHONY: start stop ps logs require-validator-key