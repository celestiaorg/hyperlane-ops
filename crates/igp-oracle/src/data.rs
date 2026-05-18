use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use alloy::providers::{Provider, ProviderBuilder};
use async_trait::async_trait;
use reqwest::Client;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::{
    adapter::{GasAdapter, PriceAdapter},
    config::MarketDataConfig,
    error::{IgpOracleError, Result},
    models::{ChainProtocol, GasPriceSample, ReconciliationTarget},
};

#[derive(Debug)]
pub struct CoinGeckoPriceAdapter {
    client: Client,
    assets: BTreeMap<String, String>,
    chain_scope: Option<BTreeSet<String>>,
    cache_ttl_seconds: u64,
    stale_after_seconds: u64,
    cache: Mutex<Option<CoinGeckoCacheEntry>>,
}

impl CoinGeckoPriceAdapter {
    pub fn new(config: &MarketDataConfig) -> Result<Self> {
        if config.provider != "coingecko" {
            return Err(IgpOracleError::InvalidConfig(format!(
                "unsupported market data provider {}",
                config.provider
            )));
        }

        Ok(Self {
            client: http_client()?,
            assets: config.assets.clone(),
            chain_scope: None,
            cache_ttl_seconds: config.cache_ttl_seconds,
            stale_after_seconds: config.stale_after_seconds,
            cache: Mutex::new(None),
        })
    }

    pub fn new_scoped(
        config: &MarketDataConfig,
        chain_scope: impl IntoIterator<Item = String>,
    ) -> Result<Self> {
        let mut adapter = Self::new(config)?;
        adapter.chain_scope = Some(chain_scope.into_iter().collect());
        Ok(adapter)
    }

    async fn prices(&self) -> Result<BTreeMap<String, CoinGeckoPrice>> {
        let now = unix_now()?;
        if let Some(cached) = self.cached_prices(now)? {
            return cached;
        }

        let fetched = self.fetch_prices().await;
        let response = match fetched {
            Ok(prices) => CoinGeckoCachedResponse::Prices(prices),
            Err(err) => CoinGeckoCachedResponse::Error(err.to_string()),
        };

        let mut cache = self.cache.lock().map_err(|_| {
            IgpOracleError::MarketData("CoinGecko price cache lock was poisoned".to_string())
        })?;
        *cache = Some(CoinGeckoCacheEntry {
            fetched_at: now,
            response,
        });

        cache
            .as_ref()
            .expect("cache entry was just written")
            .response
            .to_result()
    }

    fn cached_prices(&self, now: u64) -> Result<Option<Result<BTreeMap<String, CoinGeckoPrice>>>> {
        let cache = self.cache.lock().map_err(|_| {
            IgpOracleError::MarketData("CoinGecko price cache lock was poisoned".to_string())
        })?;
        Ok(cache
            .as_ref()
            .filter(|entry| now.saturating_sub(entry.fetched_at) <= self.cache_ttl_seconds)
            .map(|entry| entry.response.to_result()))
    }

    async fn fetch_prices(&self) -> Result<BTreeMap<String, CoinGeckoPrice>> {
        let ids = asset_ids(self.assets.iter().filter(|(chain, _)| {
            self.chain_scope
                .as_ref()
                .is_none_or(|scope| scope.contains(*chain))
        }));
        let url = "https://api.coingecko.com/api/v3/simple/price";
        self.client
            .get(url)
            .query(&[
                ("ids", ids.as_str()),
                ("vs_currencies", "usd"),
                ("include_last_updated_at", "true"),
            ])
            .send()
            .await
            .map_err(|source| {
                IgpOracleError::MarketData(format!("CoinGecko request failed: {source}"))
            })?
            .error_for_status()
            .map_err(|source| {
                IgpOracleError::MarketData(format!("CoinGecko returned an error: {source}"))
            })?
            .json()
            .await
            .map_err(|source| {
                IgpOracleError::MarketData(format!("CoinGecko response parse failed: {source}"))
            })
    }
}

#[async_trait]
impl PriceAdapter for CoinGeckoPriceAdapter {
    async fn native_token_price_usd(&self, chain_name: &str) -> Result<Decimal> {
        let asset = self.assets.get(chain_name).ok_or_else(|| {
            IgpOracleError::InvalidConfig(format!(
                "missing marketData asset for chain {chain_name}"
            ))
        })?;

        let response = self.prices().await?;

        let price = response.get(asset).ok_or_else(|| {
            IgpOracleError::MarketData(format!("CoinGecko did not return asset {asset}"))
        })?;

        if price.usd <= Decimal::ZERO {
            return Err(IgpOracleError::MarketData(format!(
                "CoinGecko did not return a positive USD price for asset {asset}"
            )));
        }

        let last_updated_at = price.last_updated_at.ok_or_else(|| {
            IgpOracleError::MarketData(format!(
                "CoinGecko did not return last_updated_at for asset {asset}"
            ))
        })?;
        let now = unix_now()?;
        if now.saturating_sub(last_updated_at) > self.stale_after_seconds {
            return Err(IgpOracleError::MarketData(format!(
                "CoinGecko price for asset {asset} is stale: last updated {last_updated_at}, now {now}"
            )));
        }

        Ok(price.usd)
    }
}

#[derive(Debug, Clone)]
struct CoinGeckoCacheEntry {
    fetched_at: u64,
    response: CoinGeckoCachedResponse,
}

#[derive(Debug, Clone)]
enum CoinGeckoCachedResponse {
    Prices(BTreeMap<String, CoinGeckoPrice>),
    Error(String),
}

impl CoinGeckoCachedResponse {
    fn to_result(&self) -> Result<BTreeMap<String, CoinGeckoPrice>> {
        match self {
            Self::Prices(prices) => Ok(prices.clone()),
            Self::Error(error) => Err(IgpOracleError::MarketData(error.clone())),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct CoinGeckoPrice {
    #[serde(with = "rust_decimal::serde::float")]
    usd: Decimal,
    last_updated_at: Option<u64>,
}

#[derive(Debug, Clone, Default)]
pub struct ProtocolGasAdapter;

impl ProtocolGasAdapter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl GasAdapter for ProtocolGasAdapter {
    async fn remote_gas_price(&self, target: &ReconciliationTarget) -> Result<GasPriceSample> {
        match target.remote.protocol {
            ChainProtocol::Ethereum => self.evm_gas_price(target).await,
            ChainProtocol::CosmosNative => registry_cosmos_gas_price(target),
        }
    }
}

impl ProtocolGasAdapter {
    async fn evm_gas_price(&self, target: &ReconciliationTarget) -> Result<GasPriceSample> {
        let rpc_url = target
            .remote
            .rpc_urls
            .first()
            .ok_or_else(|| {
                IgpOracleError::GasData(format!(
                    "remote chain {} has no rpcUrls entry",
                    target.remote.name
                ))
            })?
            .http
            .clone();

        let url = rpc_url.parse().map_err(|err| {
            IgpOracleError::GasData(format!("invalid EVM RPC URL {rpc_url}: {err}"))
        })?;
        let provider = ProviderBuilder::new().connect_http(url);
        let sampled_gas_price = provider.get_gas_price().await.map_err(|err| {
            IgpOracleError::GasData(format!(
                "eth_gasPrice failed for {}: {err}",
                target.remote.name
            ))
        })?;

        Ok(GasPriceSample {
            source: "rpc".to_string(),
            raw_amount: Some(format!("{sampled_gas_price:#x}")),
            raw_denom: Some(
                target
                    .remote
                    .native_token
                    .denom
                    .clone()
                    .unwrap_or_else(|| target.remote.native_token.symbol.clone()),
            ),
            sampled_gas_price: sampled_gas_price.to_string(),
            rounding: None,
            reason: None,
            endpoint: Some(rpc_url),
        })
    }
}

fn registry_cosmos_gas_price(target: &ReconciliationTarget) -> Result<GasPriceSample> {
    let gas_price = target.remote.gas_price.as_ref().ok_or_else(|| {
        IgpOracleError::GasData(format!(
            "remote cosmosnative chain {} has no gasPrice metadata",
            target.remote.name
        ))
    })?;

    let sampled_gas_price = decimal_str_to_ceil_u128(&gas_price.amount)?;
    let rounding = if gas_price.amount.contains('.') {
        Some("ceil".to_string())
    } else {
        None
    };
    let reason = rounding.as_ref().map(|_| {
        "IGP gasPrice is an integer; fractional registry gasPrice was rounded up".to_string()
    });

    Ok(GasPriceSample {
        source: "registry".to_string(),
        raw_amount: Some(gas_price.amount.clone()),
        raw_denom: Some(gas_price.denom.clone()),
        sampled_gas_price: sampled_gas_price.to_string(),
        rounding,
        reason,
        endpoint: None,
    })
}

pub fn decimal_str_to_ceil_u128(value: &str) -> Result<u128> {
    let decimal = Decimal::from_str(value).map_err(|source| {
        IgpOracleError::DataSource(format!("invalid decimal integer source {value}: {source}"))
    })?;
    decimal_to_ceil_u128(decimal)
}

pub fn decimal_to_ceil_u128(value: Decimal) -> Result<u128> {
    if value < Decimal::ZERO {
        return Err(IgpOracleError::DataSource(format!(
            "negative numeric value {value}"
        )));
    }

    value.ceil().to_u128().ok_or_else(|| {
        IgpOracleError::DataSource(format!("numeric value {value} does not fit in u128"))
    })
}

pub(crate) fn http_client() -> Result<Client> {
    Client::builder()
        .user_agent("igp-oracle/0.1")
        .build()
        .map_err(|source| {
            IgpOracleError::DataSource(format!("failed to build HTTP client: {source}"))
        })
}

fn unix_now() -> Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|source| IgpOracleError::DataSource(format!("system clock error: {source}")))
}

fn asset_ids<'a>(assets: impl IntoIterator<Item = (&'a String, &'a String)>) -> String {
    assets
        .into_iter()
        .map(|(_, asset)| asset)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use crate::{
        config::{GasConfig, GasMode, RemoteSelection, TargetConfig, WriteConfig, WriteMethod},
        models::{
            ChainId, ChainMetadata, ChainProtocol, CoreAddresses, MetadataGasPrice, NativeToken,
            ReconciliationTarget,
        },
        policy::clamp_config,
    };

    use super::*;

    #[test]
    fn coingecko_asset_ids_are_deduplicated_and_sorted() {
        let assets = BTreeMap::from([
            ("arbitrum".to_string(), "ethereum".to_string()),
            ("celestia".to_string(), "celestia".to_string()),
            ("ethereum".to_string(), "ethereum".to_string()),
        ]);

        assert_eq!(asset_ids(assets.iter()), "celestia,ethereum");
    }

    #[test]
    fn coingecko_asset_ids_honor_chain_scope() {
        let assets = BTreeMap::from([
            ("arbitrum".to_string(), "ethereum".to_string()),
            ("celestia".to_string(), "celestia".to_string()),
            ("ethereum".to_string(), "ethereum".to_string()),
            ("unused".to_string(), "bitcoin".to_string()),
        ]);
        let scope = BTreeSet::from(["celestia".to_string(), "ethereum".to_string()]);

        assert_eq!(
            asset_ids(
                assets
                    .iter()
                    .filter(|(chain, _)| scope.contains(chain.as_str()))
            ),
            "celestia,ethereum"
        );
    }

    #[test]
    fn rounds_decimal_gas_price_up() {
        assert_eq!(decimal_str_to_ceil_u128("0.1").expect("decimal"), 1);
        assert_eq!(decimal_str_to_ceil_u128("7.0").expect("decimal"), 7);
    }

    #[test]
    fn cosmos_registry_gas_sample_records_fractional_rounding() {
        let target = test_target_with_remote_gas_price("0.002", "utia");

        let sample = registry_cosmos_gas_price(&target).expect("gas sample");

        assert_eq!(sample.source, "registry");
        assert_eq!(sample.raw_amount.as_deref(), Some("0.002"));
        assert_eq!(sample.raw_denom.as_deref(), Some("utia"));
        assert_eq!(sample.sampled_gas_price, "1");
        assert_eq!(sample.rounding.as_deref(), Some("ceil"));
        assert_eq!(
            sample.reason.as_deref(),
            Some("IGP gasPrice is an integer; fractional registry gasPrice was rounded up")
        );
    }

    #[test]
    fn rejects_negative_decimal() {
        let err = decimal_to_ceil_u128(Decimal::from(-1)).expect_err("negative should fail");
        assert!(matches!(err, IgpOracleError::DataSource(_)));
    }

    fn test_target_with_remote_gas_price(amount: &str, denom: &str) -> ReconciliationTarget {
        ReconciliationTarget {
            origin: ChainMetadata {
                name: "ethereum".to_string(),
                domain_id: 1,
                chain_id: ChainId::Number(1),
                protocol: ChainProtocol::Ethereum,
                native_token: NativeToken {
                    name: "Ether".to_string(),
                    symbol: "ETH".to_string(),
                    decimals: 18,
                    denom: None,
                },
                rpc_urls: vec![],
                grpc_urls: vec![],
                rest_urls: vec![],
                gas_price: None,
                bech32_prefix: None,
            },
            remote: ChainMetadata {
                name: "celestia".to_string(),
                domain_id: 1_128_614_981,
                chain_id: ChainId::String("celestia".to_string()),
                protocol: ChainProtocol::CosmosNative,
                native_token: NativeToken {
                    name: "Celestia".to_string(),
                    symbol: "TIA".to_string(),
                    decimals: 6,
                    denom: Some("utia".to_string()),
                },
                rpc_urls: vec![],
                grpc_urls: vec![],
                rest_urls: vec![],
                gas_price: Some(MetadataGasPrice {
                    amount: amount.to_string(),
                    denom: denom.to_string(),
                }),
                bech32_prefix: Some("celestia".to_string()),
            },
            origin_addresses: CoreAddresses::default(),
            config: TargetConfig {
                origin_chain: "ethereum".to_string(),
                remote_selection: RemoteSelection::ConfiguredOnOriginIgp,
                enabled: true,
                gas: GasConfig {
                    mode: GasMode::Sample,
                    source: "registry".to_string(),
                    min: "1".to_string(),
                    max: "1000000000000".to_string(),
                },
                exchange_rate: clamp_config("1", "1000000000000000000000000000000"),
                write: WriteConfig {
                    enabled: false,
                    method: WriteMethod::Evm,
                    signer_profile: "test".to_string(),
                },
            },
            gas_overhead: 10_000,
        }
    }
}
