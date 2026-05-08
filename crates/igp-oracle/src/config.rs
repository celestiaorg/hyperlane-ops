use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use serde::{ser::SerializeMap, Deserialize, Deserializer, Serialize, Serializer};

use crate::{
    error::{read_to_string, IgpOracleError, Result},
    models::ChainProtocol,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MarketDataConfig {
    pub provider: String,
    pub cache_ttl_seconds: u64,
    pub stale_after_seconds: u64,
    #[serde(default)]
    pub assets: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DefaultsConfig {
    pub min_bps_change_to_write: u64,
    pub max_bps_change_per_update: u64,
    pub cooldown_seconds: u64,
    pub safety_multiplier_bps: u64,
    pub gas_sample_freshness_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TargetConfig {
    pub origin_chain: String,
    pub remote_selection: RemoteSelection,
    pub enabled: bool,
    pub gas: GasConfig,
    pub exchange_rate: ClampConfig,
    pub write: WriteConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteSelection {
    ConfiguredOnOriginIgp,
    Domains { domains: Vec<u32> },
}

impl Serialize for RemoteSelection {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::ConfiguredOnOriginIgp => serializer.serialize_str("configuredOnOriginIgp"),
            Self::Domains { domains } => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("domains", domains)?;
                map.end()
            }
        }
    }
}

impl RemoteSelection {
    pub fn operator_domains(&self) -> Option<&[u32]> {
        match self {
            Self::ConfiguredOnOriginIgp => None,
            Self::Domains { domains } => Some(domains.as_slice()),
        }
    }
}

impl<'de> Deserialize<'de> for RemoteSelection {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_yaml::Value::deserialize(deserializer)?;
        match value {
            serde_yaml::Value::String(value) if value == "configuredOnOriginIgp" => {
                Ok(Self::ConfiguredOnOriginIgp)
            }
            serde_yaml::Value::Mapping(mapping) => {
                if mapping.len() != 1 {
                    return Err(serde::de::Error::custom(
                        "remoteSelection map only supports the domains field",
                    ));
                }
                let domains = mapping
                    .get(serde_yaml::Value::String("domains".to_string()))
                    .ok_or_else(|| {
                        serde::de::Error::custom("remoteSelection map must contain a domains field")
                    })?;
                let domains: Vec<u32> =
                    serde_yaml::from_value(domains.clone()).map_err(serde::de::Error::custom)?;
                if domains.is_empty() {
                    return Err(serde::de::Error::custom(
                        "remoteSelection.domains must not be empty",
                    ));
                }
                let mut seen = BTreeSet::new();
                for domain in &domains {
                    if !seen.insert(*domain) {
                        return Err(serde::de::Error::custom(format!(
                            "remoteSelection.domains contains duplicate domain {domain}"
                        )));
                    }
                }
                Ok(Self::Domains { domains })
            }
            _ => Err(serde::de::Error::custom(
                "remoteSelection must be configuredOnOriginIgp or a map with domains",
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GasConfig {
    #[serde(default)]
    pub mode: GasMode,
    pub source: String,
    pub min: String,
    pub max: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum GasMode {
    #[default]
    Sample,
    Preserve,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClampConfig {
    pub min: String,
    pub max: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WriteConfig {
    pub enabled: bool,
    pub method: String,
    pub signer_profile: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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
    remoteSelection: configuredOnOriginIgp
    enabled: true
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
            config.targets[0].remote_selection,
            RemoteSelection::ConfiguredOnOriginIgp
        );
    }

    #[test]
    fn parses_operator_domain_remote_selection() {
        let path = Path::new("fixture.yaml");
        let config = UpdaterConfig::from_str(
            path,
            r#"
marketData:
  provider: coingecko
  cacheTtlSeconds: 60
  staleAfterSeconds: 300
defaults:
  minBpsChangeToWrite: 500
  maxBpsChangePerUpdate: 5000
  cooldownSeconds: 900
  safetyMultiplierBps: 11000
  gasSampleFreshnessSeconds: 120
targets:
  - originChain: ethereum
    remoteSelection:
      domains:
        - 1128614981
    enabled: true
    gas:
      source: rpc
      min: "1"
      max: "100"
    exchangeRate:
      min: "1"
      max: "100"
    write:
      enabled: true
      method: evm
      signerProfile: ethereum-owner
"#,
        )
        .expect("config should parse");

        assert_eq!(
            config.targets[0].remote_selection,
            RemoteSelection::Domains {
                domains: vec![1_128_614_981]
            }
        );
    }
}
