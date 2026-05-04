use rust_decimal::Decimal;

use crate::{
    adapters::{GasAdapter, PriceAdapter},
    config::{ClampConfig, DefaultsConfig},
    data::{decimal_str_to_ceil_u128, decimal_to_ceil_u128},
    error::{IgpOracleError, Result},
    models::{ProposedIgpConfig, ReconciliationTarget},
};

const TOKEN_EXCHANGE_RATE_SCALE: u64 = 10_000_000_000;
const BPS_DENOMINATOR: u64 = 10_000;

pub async fn compute_proposed_config(
    target: &ReconciliationTarget,
    defaults: &DefaultsConfig,
    gas_adapter: &dyn GasAdapter,
    price_adapter: &dyn PriceAdapter,
) -> Result<ProposedIgpConfig> {
    let remote_gas_price = gas_adapter.remote_gas_price(target).await?;
    let origin_price = price_adapter
        .native_token_price_usd(&target.origin.name)
        .await?;
    let remote_price = price_adapter
        .native_token_price_usd(&target.remote.name)
        .await?;

    if origin_price <= Decimal::ZERO || remote_price <= Decimal::ZERO {
        return Err(IgpOracleError::DataSource(format!(
            "non-positive price data for {} or {}",
            target.origin.name, target.remote.name
        )));
    }

    let gas_price = apply_bps_multiplier(remote_gas_price, defaults.safety_multiplier_bps)?;
    let gas_price = clamp_u128(gas_price, &target.config.gas.min, &target.config.gas.max)?;

    let exchange_rate = (remote_price / origin_price) * Decimal::from(TOKEN_EXCHANGE_RATE_SCALE);
    let exchange_rate =
        apply_decimal_bps_multiplier(exchange_rate, defaults.safety_multiplier_bps)?;
    let exchange_rate = clamp_u128(
        exchange_rate,
        &target.config.exchange_rate.min,
        &target.config.exchange_rate.max,
    )?;

    Ok(ProposedIgpConfig {
        gas_price: gas_price.to_string(),
        token_exchange_rate: exchange_rate.to_string(),
        gas_overhead: target.config.gas_overhead,
    })
}

fn apply_bps_multiplier(value: u128, multiplier_bps: u64) -> Result<u128> {
    let value = Decimal::from_str_exact(&value.to_string()).map_err(|source| {
        IgpOracleError::Policy(format!("failed to convert integer value {value}: {source}"))
    })?;
    apply_decimal_bps_multiplier(value, multiplier_bps)
}

fn apply_decimal_bps_multiplier(value: Decimal, multiplier_bps: u64) -> Result<u128> {
    let scaled = value * Decimal::from(multiplier_bps) / Decimal::from(BPS_DENOMINATOR);
    decimal_to_ceil_u128(scaled)
}

fn clamp_u128(value: u128, min: &str, max: &str) -> Result<u128> {
    let min = decimal_str_to_ceil_u128(min)?;
    let max = decimal_str_to_ceil_u128(max)?;
    if min > max {
        return Err(IgpOracleError::Policy(format!(
            "invalid clamp: min {min} is greater than max {max}"
        )));
    }
    Ok(value.clamp(min, max))
}

pub fn clamp_config(min: &str, max: &str) -> ClampConfig {
    ClampConfig {
        min: min.to_string(),
        max: max.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use async_trait::async_trait;

    use crate::{
        adapters::{GasAdapter, PriceAdapter},
        config::{DefaultsConfig, GasConfig, TargetConfig, WriteConfig},
        models::{
            ChainId, ChainMetadata, ChainProtocol, CoreAddresses, NativeToken, ReconciliationTarget,
        },
    };

    use super::*;

    struct StaticGasAdapter(u128);

    #[async_trait]
    impl GasAdapter for StaticGasAdapter {
        async fn remote_gas_price(&self, _target: &ReconciliationTarget) -> Result<u128> {
            Ok(self.0)
        }
    }

    struct StaticPriceAdapter(BTreeMap<String, Decimal>);

    #[async_trait]
    impl PriceAdapter for StaticPriceAdapter {
        async fn native_token_price_usd(&self, chain_name: &str) -> Result<Decimal> {
            self.0.get(chain_name).copied().ok_or_else(|| {
                IgpOracleError::DataSource(format!("missing static price for {chain_name}"))
            })
        }
    }

    #[tokio::test]
    async fn computes_proposed_config_with_safety_multiplier() {
        let target = target();
        let defaults = DefaultsConfig {
            min_bps_change_to_write: 500,
            max_bps_change_per_update: 5000,
            cooldown_seconds: 900,
            safety_multiplier_bps: 11_000,
            gas_sample_freshness_seconds: 120,
        };
        let mut prices = BTreeMap::new();
        prices.insert("origin".to_string(), Decimal::from(2));
        prices.insert("remote".to_string(), Decimal::from(4));

        let proposed = compute_proposed_config(
            &target,
            &defaults,
            &StaticGasAdapter(100),
            &StaticPriceAdapter(prices),
        )
        .await
        .expect("proposal");

        assert_eq!(proposed.gas_price, "110");
        assert_eq!(proposed.token_exchange_rate, "22000000000");
        assert_eq!(proposed.gas_overhead, 174_289);
    }

    fn target() -> ReconciliationTarget {
        ReconciliationTarget {
            origin: chain("origin", ChainProtocol::CosmosNative, 1),
            remote: chain("remote", ChainProtocol::Ethereum, 2),
            origin_addresses: CoreAddresses::default(),
            config: TargetConfig {
                origin_chain: "origin".to_string(),
                remote_chain: Some("remote".to_string()),
                remote_domain: None,
                enabled: true,
                gas_overhead: 174_289,
                gas: GasConfig {
                    source: "rpc".to_string(),
                    min: "1".to_string(),
                    max: "1000000000000".to_string(),
                },
                exchange_rate: clamp_config("1", "1000000000000000"),
                write: WriteConfig {
                    enabled: true,
                    method: "celestia-grpc".to_string(),
                    signer_profile: "owner".to_string(),
                },
            },
        }
    }

    fn chain(name: &str, protocol: ChainProtocol, domain_id: u32) -> ChainMetadata {
        ChainMetadata {
            name: name.to_string(),
            domain_id,
            chain_id: ChainId::Number(domain_id as u64),
            protocol,
            native_token: NativeToken {
                name: "Token".to_string(),
                symbol: "TKN".to_string(),
                decimals: 18,
                denom: None,
            },
            rpc_urls: Vec::new(),
            grpc_urls: Vec::new(),
            rest_urls: Vec::new(),
            gas_price: None,
            bech32_prefix: None,
        }
    }
}
