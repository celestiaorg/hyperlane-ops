COMPOSE := docker compose -f docker-compose.yml

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

.PHONY: start stop ps logs require-validator-key