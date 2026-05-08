use rust_decimal::Decimal;

use crate::{
    adapter::{GasAdapter, PriceAdapter},
    config::{ClampConfig, DefaultsConfig, GasMode},
    data::{decimal_str_to_ceil_u128, decimal_to_ceil_u128},
    error::{IgpOracleError, Result},
    models::{
        CurrentIgpConfig, ProposalComputation, ProposedIgpConfig, ReconciliationDelta,
        ReconciliationTarget,
    },
};

const TOKEN_EXCHANGE_RATE_SCALE: u64 = 10_000_000_000;
const BPS_DENOMINATOR: u64 = 10_000;

pub async fn compute_proposed_config(
    target: &ReconciliationTarget,
    current: &CurrentIgpConfig,
    defaults: &DefaultsConfig,
    gas_adapter: &dyn GasAdapter,
    price_adapter: &dyn PriceAdapter,
) -> Result<ProposedIgpConfig> {
    Ok(
        compute_proposal(target, current, defaults, gas_adapter, price_adapter)
            .await?
            .proposed,
    )
}

pub async fn compute_proposal(
    target: &ReconciliationTarget,
    current: &CurrentIgpConfig,
    defaults: &DefaultsConfig,
    gas_adapter: &dyn GasAdapter,
    price_adapter: &dyn PriceAdapter,
) -> Result<ProposalComputation> {
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

    let (gas_sample, gas_price) = match target.config.gas.mode {
        GasMode::Sample => {
            let sample = gas_adapter.remote_gas_price(target).await?;
            let remote_gas_price = decimal_str_to_ceil_u128(&sample.sampled_gas_price)?;
            let gas_price = apply_bps_multiplier(remote_gas_price, defaults.safety_multiplier_bps)?;
            let gas_price = clamp_u128(gas_price, &target.config.gas.min, &target.config.gas.max)?;
            (Some(sample), gas_price.to_string())
        }
        GasMode::Preserve => {
            parse_u128(&current.gas_price, "current gasPrice")?;
            (None, current.gas_price.clone())
        }
    };

    let token_decimal_adjustment = decimal_power_of_ten(
        target.origin.native_token.decimals as i16 - target.remote.native_token.decimals as i16,
    )?;
    let exchange_rate = (remote_price / origin_price)
        * token_decimal_adjustment
        * Decimal::from(TOKEN_EXCHANGE_RATE_SCALE);
    let exchange_rate =
        apply_decimal_bps_multiplier(exchange_rate, defaults.safety_multiplier_bps)?;
    let exchange_rate = clamp_u128(
        exchange_rate,
        &target.config.exchange_rate.min,
        &target.config.exchange_rate.max,
    )?;

    Ok(ProposalComputation {
        proposed: ProposedIgpConfig {
            gas_price,
            token_exchange_rate: exchange_rate.to_string(),
            gas_overhead: target.gas_overhead,
        },
        gas: gas_sample,
        gas_mode: match target.config.gas.mode {
            GasMode::Sample => "sample",
            GasMode::Preserve => "preserve",
        }
        .to_string(),
        origin_price_usd: origin_price.to_string(),
        remote_price_usd: remote_price.to_string(),
        origin_native_token_decimals: target.origin.native_token.decimals,
        remote_native_token_decimals: target.remote.native_token.decimals,
        token_decimal_adjustment: token_decimal_adjustment.to_string(),
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

fn decimal_power_of_ten(exponent: i16) -> Result<Decimal> {
    let mut value = Decimal::ONE;
    let ten = Decimal::from(10u64);

    if exponent >= 0 {
        for _ in 0..exponent {
            value = value.checked_mul(ten).ok_or_else(|| {
                IgpOracleError::Policy(format!("10^{exponent} overflows decimal arithmetic"))
            })?;
        }
    } else {
        for _ in 0..exponent.abs() {
            value = value.checked_div(ten).ok_or_else(|| {
                IgpOracleError::Policy(format!("10^{exponent} underflows decimal arithmetic"))
            })?;
        }
    }

    Ok(value)
}

pub fn clamp_config(min: &str, max: &str) -> ClampConfig {
    ClampConfig {
        min: min.to_string(),
        max: max.to_string(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconciliationDecision {
    pub status: DecisionStatus,
    pub code: DecisionCode,
    pub reason: String,
    pub deltas: ReconciliationDelta,
    pub field: Option<ReconciliationField>,
    pub observed_delta_bps: Option<u128>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionStatus {
    Noop,
    UpdateRecommended,
    PolicyViolation,
}

impl DecisionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Noop => "noop",
            Self::UpdateRecommended => "update_recommended",
            Self::PolicyViolation => "policy_violation",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionCode {
    Noop,
    UpdateThresholdMet,
    LargeDeltaRequiresManualReview,
    ZeroCurrentValueRequiresManualReview,
}

impl DecisionCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Noop => "noop",
            Self::UpdateThresholdMet => "update_threshold_met",
            Self::LargeDeltaRequiresManualReview => "large_delta_requires_manual_review",
            Self::ZeroCurrentValueRequiresManualReview => {
                "zero_current_value_requires_manual_review"
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconciliationField {
    GasPrice,
    TokenExchangeRate,
    GasOverhead,
}

impl ReconciliationField {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GasPrice => "gasPrice",
            Self::TokenExchangeRate => "tokenExchangeRate",
            Self::GasOverhead => "gasOverhead",
        }
    }
}

pub fn decide_reconciliation(
    current: &CurrentIgpConfig,
    proposed: &ProposedIgpConfig,
    defaults: &DefaultsConfig,
) -> Result<ReconciliationDecision> {
    let current_gas_price = parse_u128(&current.gas_price, "current gasPrice")?;
    let proposed_gas_price = parse_u128(&proposed.gas_price, "proposed gasPrice")?;
    let current_exchange_rate =
        parse_u128(&current.token_exchange_rate, "current tokenExchangeRate")?;
    let proposed_exchange_rate =
        parse_u128(&proposed.token_exchange_rate, "proposed tokenExchangeRate")?;

    let deltas = ReconciliationDelta {
        gas_price_bps: delta_bps(current_gas_price, proposed_gas_price),
        token_exchange_rate_bps: delta_bps(current_exchange_rate, proposed_exchange_rate),
        gas_overhead_bps: delta_bps(current.gas_overhead as u128, proposed.gas_overhead as u128),
    };

    if let Some(field) = zero_current_field(
        current_gas_price,
        current_exchange_rate,
        current.gas_overhead,
        proposed.gas_overhead,
    ) {
        return Ok(ReconciliationDecision {
            status: DecisionStatus::PolicyViolation,
            code: DecisionCode::ZeroCurrentValueRequiresManualReview,
            reason: format!(
                "current on-chain {} is zero; manual review required",
                field.as_str()
            ),
            deltas,
            field: Some(field),
            observed_delta_bps: None,
        });
    }

    let (field, max_delta) = max_delta(&deltas).unwrap_or((ReconciliationField::GasPrice, 0));

    if max_delta > defaults.max_bps_change_per_update as u128 {
        return Ok(ReconciliationDecision {
            status: DecisionStatus::PolicyViolation,
            code: DecisionCode::LargeDeltaRequiresManualReview,
            reason: format!(
                "{} delta {max_delta} bps exceeds configured max {} bps",
                field.as_str(),
                defaults.max_bps_change_per_update
            ),
            deltas,
            field: Some(field),
            observed_delta_bps: Some(max_delta),
        });
    }

    if max_delta >= defaults.min_bps_change_to_write as u128 {
        return Ok(ReconciliationDecision {
            status: DecisionStatus::UpdateRecommended,
            code: DecisionCode::UpdateThresholdMet,
            reason: format!(
                "{} delta {max_delta} bps meets configured write threshold {} bps",
                field.as_str(),
                defaults.min_bps_change_to_write
            ),
            deltas,
            field: Some(field),
            observed_delta_bps: Some(max_delta),
        });
    }

    Ok(ReconciliationDecision {
        status: DecisionStatus::Noop,
        code: DecisionCode::Noop,
        reason: format!(
            "{} delta {max_delta} bps is below configured write threshold {} bps",
            field.as_str(),
            defaults.min_bps_change_to_write
        ),
        deltas,
        field: Some(field),
        observed_delta_bps: Some(max_delta),
    })
}

fn parse_u128(value: &str, label: &str) -> Result<u128> {
    value
        .parse::<u128>()
        .map_err(|source| IgpOracleError::Policy(format!("invalid {label} {value}: {source}")))
}

fn delta_bps(current: u128, proposed: u128) -> Option<u128> {
    if current == 0 {
        return None;
    }
    Some(current.abs_diff(proposed) * BPS_DENOMINATOR as u128 / current)
}

fn zero_current_field(
    gas_price: u128,
    token_exchange_rate: u128,
    gas_overhead: u64,
    proposed_gas_overhead: u64,
) -> Option<ReconciliationField> {
    if gas_price == 0 {
        return Some(ReconciliationField::GasPrice);
    }
    if token_exchange_rate == 0 {
        return Some(ReconciliationField::TokenExchangeRate);
    }
    if gas_overhead == 0 && proposed_gas_overhead > 0 {
        return Some(ReconciliationField::GasOverhead);
    }
    None
}

fn max_delta(deltas: &ReconciliationDelta) -> Option<(ReconciliationField, u128)> {
    [
        (ReconciliationField::GasPrice, deltas.gas_price_bps),
        (
            ReconciliationField::TokenExchangeRate,
            deltas.token_exchange_rate_bps,
        ),
        (ReconciliationField::GasOverhead, deltas.gas_overhead_bps),
    ]
    .into_iter()
    .filter_map(|(field, delta)| delta.map(|delta| (field, delta)))
    .max_by_key(|(_, delta)| *delta)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use async_trait::async_trait;
    use serde::Deserialize;

    use crate::{
        adapter::{GasAdapter, PriceAdapter},
        config::{DefaultsConfig, GasConfig, GasMode, TargetConfig, WriteConfig},
        models::{
            ChainId, ChainMetadata, ChainProtocol, CoreAddresses, GasPriceSample, NativeToken,
            ReconciliationTarget,
        },
    };

    use super::*;

    struct StaticGasAdapter(u128);

    #[async_trait]
    impl GasAdapter for StaticGasAdapter {
        async fn remote_gas_price(&self, target: &ReconciliationTarget) -> Result<GasPriceSample> {
            Ok(GasPriceSample {
                source: "test".to_string(),
                remote_chain: target.remote.name.clone(),
                raw_amount: Some(self.0.to_string()),
                raw_denom: None,
                sampled_gas_price: self.0.to_string(),
                rounding: None,
                reason: None,
                endpoint: None,
            })
        }
    }

    struct FailingGasAdapter;

    #[async_trait]
    impl GasAdapter for FailingGasAdapter {
        async fn remote_gas_price(&self, _target: &ReconciliationTarget) -> Result<GasPriceSample> {
            Err(IgpOracleError::DataSource(
                "gas adapter should not be called".to_string(),
            ))
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

    #[derive(Debug, Deserialize)]
    struct SampleConfigs {
        destination_gas_configs: Vec<SampleDestinationGasConfig>,
    }

    #[derive(Debug, Deserialize)]
    struct SampleDestinationGasConfig {
        remote_domain: u32,
        gas_oracle: SampleGasOracle,
        gas_overhead: String,
    }

    #[derive(Debug, Deserialize)]
    struct SampleGasOracle {
        token_exchange_rate: String,
        gas_price: String,
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
            &current_config(),
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

    #[tokio::test]
    async fn computes_exchange_rate_with_native_token_decimals() {
        let target = target_with_decimals(6, 18);
        let defaults = DefaultsConfig {
            min_bps_change_to_write: 500,
            max_bps_change_per_update: 5000,
            cooldown_seconds: 900,
            safety_multiplier_bps: 10_000,
            gas_sample_freshness_seconds: 120,
        };
        let mut prices = BTreeMap::new();
        prices.insert(
            "origin".to_string(),
            Decimal::from_str_exact("0.25").unwrap(),
        );
        prices.insert("remote".to_string(), Decimal::from(2500));

        let proposal = compute_proposal(
            &target,
            &current_config(),
            &defaults,
            &StaticGasAdapter(300_000_000),
            &StaticPriceAdapter(prices),
        )
        .await
        .expect("proposal");

        assert_eq!(proposal.proposed.gas_price, "300000000");
        assert_eq!(proposal.proposed.token_exchange_rate, "100");
        assert_eq!(proposal.proposed.gas_overhead, 174_289);
        assert_eq!(proposal.origin_native_token_decimals, 6);
        assert_eq!(proposal.remote_native_token_decimals, 18);
        assert_eq!(proposal.token_decimal_adjustment, "0.000000000001");
    }

    #[tokio::test]
    async fn preserve_gas_mode_reuses_current_gas_price_without_sampling() {
        let mut target = target_with_decimals(18, 6);
        target.config.gas.mode = GasMode::Preserve;
        let defaults = DefaultsConfig {
            min_bps_change_to_write: 500,
            max_bps_change_per_update: 5000,
            cooldown_seconds: 900,
            safety_multiplier_bps: 10_000,
            gas_sample_freshness_seconds: 120,
        };
        let current = CurrentIgpConfig {
            gas_price: "300000000".to_string(),
            token_exchange_rate: "1600000000000000000".to_string(),
            gas_overhead: 10_000,
        };
        let mut prices = BTreeMap::new();
        prices.insert("origin".to_string(), Decimal::from(2));
        prices.insert("remote".to_string(), Decimal::from(4));

        let proposal = compute_proposal(
            &target,
            &current,
            &defaults,
            &FailingGasAdapter,
            &StaticPriceAdapter(prices),
        )
        .await
        .expect("proposal");
        let decision =
            decide_reconciliation(&current, &proposal.proposed, &defaults).expect("decision");

        assert_eq!(proposal.proposed.gas_price, "300000000");
        assert_eq!(proposal.gas_mode, "preserve");
        assert!(proposal.gas.is_none());
        assert_eq!(decision.deltas.gas_price_bps, Some(0));
    }

    #[test]
    fn parses_celestia_sample_domain_one_reference_config() {
        let sample = celestia_sample_configs();
        let domain_one = sample
            .destination_gas_configs
            .iter()
            .find(|config| config.remote_domain == 1)
            .expect("domain 1 should exist");

        assert_eq!(sample.destination_gas_configs.len(), 141);
        assert_eq!(domain_one.gas_oracle.token_exchange_rate, "101");
        assert_eq!(domain_one.gas_oracle.gas_price, "300000000");
        assert_eq!(domain_one.gas_overhead, "174289");
    }

    #[test]
    fn celestia_sample_overhead_is_reference_policy_not_universal_target() {
        let sample = celestia_sample_configs();
        let default_overhead_count = sample
            .destination_gas_configs
            .iter()
            .filter(|config| config.gas_overhead == "174289")
            .count();

        assert_eq!(default_overhead_count, 117);
        assert!(default_overhead_count < sample.destination_gas_configs.len());
    }

    #[tokio::test]
    async fn decimal_adjusted_proposal_matches_celestia_sample_order_of_magnitude() {
        let sample = celestia_sample_configs();
        let domain_one = sample
            .destination_gas_configs
            .iter()
            .find(|config| config.remote_domain == 1)
            .expect("domain 1 should exist");
        let reference_exchange_rate = domain_one
            .gas_oracle
            .token_exchange_rate
            .parse::<u128>()
            .expect("reference exchange rate should parse");
        let reference_gas_price = domain_one
            .gas_oracle
            .gas_price
            .parse::<u128>()
            .expect("reference gas price should parse");

        let target = target_with_decimals(6, 18);
        let defaults = DefaultsConfig {
            min_bps_change_to_write: 500,
            max_bps_change_per_update: 5000,
            cooldown_seconds: 900,
            safety_multiplier_bps: 10_000,
            gas_sample_freshness_seconds: 120,
        };
        let mut prices = BTreeMap::new();
        prices.insert(
            "origin".to_string(),
            Decimal::from_str_exact("0.25").unwrap(),
        );
        prices.insert("remote".to_string(), Decimal::from(2500));

        let proposal = compute_proposal(
            &target,
            &current_config(),
            &defaults,
            &StaticGasAdapter(reference_gas_price),
            &StaticPriceAdapter(prices),
        )
        .await
        .expect("proposal");
        let proposed_exchange_rate = proposal
            .proposed
            .token_exchange_rate
            .parse::<u128>()
            .expect("proposed exchange rate should parse");

        assert_eq!(proposal.proposed.gas_price, "300000000");
        assert_eq!(proposal.proposed.gas_overhead, 174_289);
        assert!(proposed_exchange_rate.abs_diff(reference_exchange_rate) <= 1);
    }

    #[test]
    fn recommends_update_when_delta_exceeds_min_threshold() {
        let defaults = DefaultsConfig {
            min_bps_change_to_write: 500,
            max_bps_change_per_update: 5000,
            cooldown_seconds: 900,
            safety_multiplier_bps: 11_000,
            gas_sample_freshness_seconds: 120,
        };
        let current = CurrentIgpConfig {
            gas_price: "100".to_string(),
            token_exchange_rate: "100".to_string(),
            gas_overhead: 100,
        };
        let proposed = ProposedIgpConfig {
            gas_price: "110".to_string(),
            token_exchange_rate: "100".to_string(),
            gas_overhead: 100,
        };

        let decision = decide_reconciliation(&current, &proposed, &defaults).expect("decision");
        assert_eq!(decision.status, DecisionStatus::UpdateRecommended);
        assert_eq!(decision.deltas.gas_price_bps, Some(1000));
    }

    #[test]
    fn flags_policy_violation_when_delta_exceeds_max_threshold() {
        let defaults = DefaultsConfig {
            min_bps_change_to_write: 500,
            max_bps_change_per_update: 5000,
            cooldown_seconds: 900,
            safety_multiplier_bps: 11_000,
            gas_sample_freshness_seconds: 120,
        };
        let current = CurrentIgpConfig {
            gas_price: "100".to_string(),
            token_exchange_rate: "100".to_string(),
            gas_overhead: 100,
        };
        let proposed = ProposedIgpConfig {
            gas_price: "10000".to_string(),
            token_exchange_rate: "100".to_string(),
            gas_overhead: 100,
        };

        let decision = decide_reconciliation(&current, &proposed, &defaults).expect("decision");
        assert_eq!(decision.status, DecisionStatus::PolicyViolation);
    }

    fn target() -> ReconciliationTarget {
        target_with_decimals(18, 18)
    }

    fn celestia_sample_configs() -> SampleConfigs {
        serde_json::from_str(include_str!("../sample-configs.celestia.json"))
            .expect("sample configs should parse")
    }

    fn target_with_decimals(origin_decimals: u8, remote_decimals: u8) -> ReconciliationTarget {
        ReconciliationTarget {
            origin: chain("origin", ChainProtocol::CosmosNative, 1, origin_decimals),
            remote: chain("remote", ChainProtocol::Ethereum, 2, remote_decimals),
            origin_addresses: CoreAddresses::default(),
            config: TargetConfig {
                origin_chain: "origin".to_string(),
                remote_selection: crate::config::RemoteSelection::ConfiguredOnOriginIgp,
                enabled: true,
                gas: GasConfig {
                    mode: GasMode::Sample,
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
            gas_overhead: 174_289,
        }
    }

    fn current_config() -> CurrentIgpConfig {
        CurrentIgpConfig {
            gas_price: "100".to_string(),
            token_exchange_rate: "1".to_string(),
            gas_overhead: 174_289,
        }
    }

    fn chain(
        name: &str,
        protocol: ChainProtocol,
        domain_id: u32,
        native_token_decimals: u8,
    ) -> ChainMetadata {
        ChainMetadata {
            name: name.to_string(),
            domain_id,
            chain_id: ChainId::Number(domain_id as u64),
            protocol,
            native_token: NativeToken {
                name: "Token".to_string(),
                symbol: "TKN".to_string(),
                decimals: native_token_decimals,
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
