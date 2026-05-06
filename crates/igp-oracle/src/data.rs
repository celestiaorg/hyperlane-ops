use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use reqwest::Client;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::{
    adapter::{GasAdapter, PriceAdapter},
    config::MarketDataConfig,
    error::{IgpOracleError, Result},
    models::{ChainProtocol, ReconciliationTarget},
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
            IgpOracleError::DataSource("CoinGecko price cache lock was poisoned".to_string())
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
            IgpOracleError::DataSource("CoinGecko price cache lock was poisoned".to_string())
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
                IgpOracleError::DataSource(format!("CoinGecko request failed: {source}"))
            })?
            .error_for_status()
            .map_err(|source| {
                IgpOracleError::DataSource(format!("CoinGecko returned an error: {source}"))
            })?
            .json()
            .await
            .map_err(|source| {
                IgpOracleError::DataSource(format!("CoinGecko response parse failed: {source}"))
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
            IgpOracleError::DataSource(format!("CoinGecko did not return asset {asset}"))
        })?;

        if price.usd <= Decimal::ZERO {
            return Err(IgpOracleError::DataSource(format!(
                "CoinGecko did not return a positive USD price for asset {asset}"
            )));
        }

        let last_updated_at = price.last_updated_at.ok_or_else(|| {
            IgpOracleError::DataSource(format!(
                "CoinGecko did not return last_updated_at for asset {asset}"
            ))
        })?;
        let now = unix_now()?;
        if now.saturating_sub(last_updated_at) > self.stale_after_seconds {
            return Err(IgpOracleError::DataSource(format!(
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
            Self::Error(error) => Err(IgpOracleError::DataSource(error.clone())),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct CoinGeckoPrice {
    #[serde(with = "rust_decimal::serde::float")]
    usd: Decimal,
    last_updated_at: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct ProtocolGasAdapter {
    client: Client,
}

impl ProtocolGasAdapter {
    pub fn new() -> Self {
        Self {
            client: http_client().expect("static user agent must build a reqwest client"),
        }
    }
}

impl Default for ProtocolGasAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl GasAdapter for ProtocolGasAdapter {
    async fn remote_gas_price(&self, target: &ReconciliationTarget) -> Result<u128> {
        match target.remote.protocol {
            ChainProtocol::Ethereum => self.evm_gas_price(target).await,
            ChainProtocol::CosmosNative => registry_cosmos_gas_price(target),
        }
    }
}

impl ProtocolGasAdapter {
    async fn evm_gas_price(&self, target: &ReconciliationTarget) -> Result<u128> {
        let rpc_url = target
            .remote
            .rpc_urls
            .first()
            .ok_or_else(|| {
                IgpOracleError::DataSource(format!(
                    "remote chain {} has no rpcUrls entry",
                    target.remote.name
                ))
            })?
            .http
            .clone();

        let response: JsonRpcResponse = self
            .client
            .post(&rpc_url)
            .json(&JsonRpcRequest {
                jsonrpc: "2.0",
                method: "eth_gasPrice",
                params: Vec::<String>::new(),
                id: 1,
            })
            .send()
            .await
            .map_err(|source| {
                IgpOracleError::DataSource(format!(
                    "eth_gasPrice request failed for {}: {source}",
                    target.remote.name
                ))
            })?
            .error_for_status()
            .map_err(|source| {
                IgpOracleError::DataSource(format!(
                    "eth_gasPrice HTTP error for {}: {source}",
                    target.remote.name
                ))
            })?
            .json()
            .await
            .map_err(|source| {
                IgpOracleError::DataSource(format!(
                    "eth_gasPrice response parse failed for {}: {source}",
                    target.remote.name
                ))
            })?;

        if let Some(error) = response.error {
            return Err(IgpOracleError::DataSource(format!(
                "eth_gasPrice RPC error for {}: {}",
                target.remote.name, error.message
            )));
        }

        parse_hex_u128(response.result.as_deref().ok_or_else(|| {
            IgpOracleError::DataSource(format!(
                "eth_gasPrice response for {} had no result",
                target.remote.name
            ))
        })?)
    }
}

fn registry_cosmos_gas_price(target: &ReconciliationTarget) -> Result<u128> {
    let gas_price = target.remote.gas_price.as_ref().ok_or_else(|| {
        IgpOracleError::DataSource(format!(
            "remote cosmosnative chain {} has no gasPrice metadata",
            target.remote.name
        ))
    })?;

    decimal_str_to_ceil_u128(&gas_price.amount)
}

pub fn parse_hex_u128(value: &str) -> Result<u128> {
    let value = value.strip_prefix("0x").unwrap_or(value);
    u128::from_str_radix(value, 16).map_err(|source| {
        IgpOracleError::DataSource(format!("invalid hex integer {value}: {source}"))
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

#[derive(Debug, Serialize)]
struct JsonRpcRequest<'a, T> {
    jsonrpc: &'a str,
    method: &'a str,
    params: T,
    id: u64,
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    result: Option<String>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    message: String,
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

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
    fn parses_evm_hex_gas_price() {
        assert_eq!(parse_hex_u128("0x3b9aca00").expect("hex"), 1_000_000_000);
    }

    #[test]
    fn rounds_decimal_gas_price_up() {
        assert_eq!(decimal_str_to_ceil_u128("0.1").expect("decimal"), 1);
        assert_eq!(decimal_str_to_ceil_u128("7.0").expect("decimal"), 7);
    }

    #[test]
    fn rejects_negative_decimal() {
        let err = decimal_to_ceil_u128(Decimal::from(-1)).expect_err("negative should fail");
        assert!(matches!(err, IgpOracleError::DataSource(_)));
    }
}
