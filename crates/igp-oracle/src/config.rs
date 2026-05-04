use std::{collections::BTreeMap, path::Path};

use serde::{Deserialize, Serialize};

use crate::{
    error::{read_to_string, IgpOracleError, Result},
    models::ChainProtocol,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdaterConfig {
    pub market_data: MarketDataConfig,
    pub defaults: DefaultsConfig,
    pub targets: Vec<TargetConfig>,
    #[serde(default)]
    pub signers: BTreeMap<String, SignerConfig>,
}

impl UpdaterConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let raw = read_to_string(path.to_path_buf())?;
        Self::from_str(path, &raw)
    }

    pub fn from_str(path: &Path, raw: &str) -> Result<Self> {
        let config: Self = serde_yaml::from_str(raw).map_err(|source| IgpOracleError::Yaml {
            path: path.to_path_buf(),
            source,
        })?;

        if config.targets.is_empty() {
            return Err(IgpOracleError::InvalidConfig(
                "at least one target must be configured".to_string(),
            ));
        }

        Ok(config)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketDataConfig {
    pub provider: String,
    pub cache_ttl_seconds: u64,
    pub stale_after_seconds: u64,
    #[serde(default)]
    pub assets: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultsConfig {
    pub min_bps_change_to_write: u64,
    pub max_bps_change_per_update: u64,
    pub cooldown_seconds: u64,
    pub safety_multiplier_bps: u64,
    pub gas_sample_freshness_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetConfig {
    pub origin_chain: String,
    pub remote_chain: Option<String>,
    pub remote_domain: Option<u32>,
    pub enabled: bool,
    pub gas_overhead: u64,
    pub gas: GasConfig,
    pub exchange_rate: ClampConfig,
    pub write: WriteConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GasConfig {
    pub source: String,
    pub min: String,
    pub max: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClampConfig {
    pub min: String,
    pub max: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteConfig {
    pub enabled: bool,
    pub method: String,
    pub signer_profile: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignerConfig {
    pub protocol: ChainProtocol,
    pub from: String,
    pub key_env: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_updater_config() {
        let path = Path::new("fixture.yaml");
        let config = UpdaterConfig::from_str(
            path,
            r#"
marketData:
  provider: coingecko
  cacheTtlSeconds: 60
  staleAfterSeconds: 300
  assets:
    celestiatestnet: celestia
defaults:
  minBpsChangeToWrite: 500
  maxBpsChangePerUpdate: 5000
  cooldownSeconds: 900
  safetyMultiplierBps: 11000
  gasSampleFreshnessSeconds: 120
targets:
  - originChain: celestiatestnet
    remoteChain: edentestnet
    enabled: true
    gasOverhead: 174289
    gas:
      source: rpc
      min: "1"
      max: "100"
    exchangeRate:
      min: "1"
      max: "100"
    write:
      enabled: true
      method: celestia-grpc
      signerProfile: celestia-owner
"#,
        )
        .expect("config should parse");

        assert_eq!(config.targets.len(), 1);
        assert_eq!(config.targets[0].origin_chain, "celestiatestnet");
        assert_eq!(
            config.targets[0].remote_chain.as_deref(),
            Some("edentestnet")
        );
    }
}
