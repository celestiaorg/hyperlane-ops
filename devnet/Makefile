.PHONY: up start stop down restart ps logs require-validator-key

COMPOSE := docker compose -f docker-compose.yml

require-validator-key:
	@test -n "$(HYP_VALIDATOR_CHECKPOINT_KEY)" || \
		{ echo "HYP_VALIDATOR_CHECKPOINT_KEY must be set"; exit 1; }

up: require-validator-key
	@$(COMPOSE) up -d

start: require-validator-key
	@$(COMPOSE) start

stop:
	@$(COMPOSE) stop

down:
	@$(COMPOSE) down

restart: stop start

ps:
	@$(COMPOSE) ps

logs:
	@$(COMPOSE) logs -f
