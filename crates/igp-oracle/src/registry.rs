use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use crate::{
    error::{read_to_string, IgpOracleError, Result},
    models::{ChainMetadata, CoreAddresses},
};

#[derive(Debug, Clone)]
pub struct RegistryLoader {
    root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct RegistryIndex {
    metadata_by_name: BTreeMap<String, ChainMetadata>,
    metadata_by_domain: BTreeMap<u32, String>,
    addresses_by_name: BTreeMap<String, CoreAddresses>,
}

impl RegistryLoader {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn load_chain_metadata(&self, chain: &str) -> Result<ChainMetadata> {
        let path = self.chain_path(chain).join("metadata.yaml");
        parse_chain_metadata(&path, &read_to_string(path.clone())?)
    }

    pub fn load_core_addresses(&self, chain: &str) -> Result<CoreAddresses> {
        let path = self.chain_path(chain).join("addresses.yaml");
        if !path.exists() {
            return Ok(CoreAddresses::default());
        }
        parse_core_addresses(&path, &read_to_string(path.clone())?)
    }

    pub fn find_chain_by_domain(&self, domain: u32) -> Result<ChainMetadata> {
        self.try_find_chain_by_domain(domain)?.ok_or_else(|| {
            IgpOracleError::Registry(format!("no chain metadata found for domain {domain}"))
        })
    }

    pub fn try_find_chain_by_domain(&self, domain: u32) -> Result<Option<ChainMetadata>> {
        let chains_dir = self.root.join("chains");
        let entries = std::fs::read_dir(&chains_dir).map_err(|source| IgpOracleError::Io {
            path: chains_dir.clone(),
            source,
        })?;

        for entry in entries {
            let entry = entry.map_err(|source| IgpOracleError::Io {
                path: chains_dir.clone(),
                source,
            })?;
            if !entry
                .file_type()
                .map_err(|source| IgpOracleError::Io {
                    path: entry.path(),
                    source,
                })?
                .is_dir()
            {
                continue;
            }

            let metadata_path = entry.path().join("metadata.yaml");
            if !metadata_path.exists() {
                continue;
            }

            let metadata =
                parse_chain_metadata(&metadata_path, &read_to_string(metadata_path.clone())?)?;
            if metadata.domain_id == domain {
                return Ok(Some(metadata));
            }
        }

        Ok(None)
    }

    fn chain_path(&self, chain: &str) -> PathBuf {
        self.root.join("chains").join(chain)
    }
}

impl RegistryIndex {
    pub fn load(root: impl Into<PathBuf>) -> Result<Self> {
        let loader = RegistryLoader::new(root);
        let chains_dir = loader.root.join("chains");
        let entries = std::fs::read_dir(&chains_dir).map_err(|source| IgpOracleError::Io {
            path: chains_dir.clone(),
            source,
        })?;

        let mut metadata_by_name = BTreeMap::new();
        let mut metadata_by_domain = BTreeMap::new();
        let mut addresses_by_name = BTreeMap::new();

        for entry in entries {
            let entry = entry.map_err(|source| IgpOracleError::Io {
                path: chains_dir.clone(),
                source,
            })?;
            if !entry
                .file_type()
                .map_err(|source| IgpOracleError::Io {
                    path: entry.path(),
                    source,
                })?
                .is_dir()
            {
                continue;
            }

            let chain_dir = entry.path();
            let metadata_path = chain_dir.join("metadata.yaml");
            if !metadata_path.exists() {
                continue;
            }

            let metadata =
                parse_chain_metadata(&metadata_path, &read_to_string(metadata_path.clone())?)?;
            let name = metadata.name.clone();

            if metadata_by_name.contains_key(&name) {
                return Err(IgpOracleError::Registry(format!(
                    "duplicate chain metadata name {name}"
                )));
            }

            if let Some(existing_name) = metadata_by_domain.get(&metadata.domain_id) {
                return Err(IgpOracleError::Registry(format!(
                    "duplicate domain {} for chains {} and {}",
                    metadata.domain_id, existing_name, name
                )));
            }

            let addresses_path = chain_dir.join("addresses.yaml");
            let addresses = if addresses_path.exists() {
                parse_core_addresses(&addresses_path, &read_to_string(addresses_path.clone())?)?
            } else {
                CoreAddresses::default()
            };
            metadata_by_domain.insert(metadata.domain_id, name.clone());
            metadata_by_name.insert(name.clone(), metadata);
            addresses_by_name.insert(name, addresses);
        }

        Ok(Self {
            metadata_by_name,
            metadata_by_domain,
            addresses_by_name,
        })
    }

    pub fn chain_metadata(&self, chain: &str) -> Result<ChainMetadata> {
        self.metadata_by_name
            .get(chain)
            .cloned()
            .ok_or_else(|| IgpOracleError::Registry(format!("no chain metadata found for {chain}")))
    }

    pub fn core_addresses(&self, chain: &str) -> Result<CoreAddresses> {
        self.addresses_by_name
            .get(chain)
            .cloned()
            .ok_or_else(|| IgpOracleError::Registry(format!("no chain metadata found for {chain}")))
    }

    pub fn chain_by_domain(&self, domain: u32) -> Result<ChainMetadata> {
        self.try_chain_by_domain(domain).ok_or_else(|| {
            IgpOracleError::Registry(format!("no chain metadata found for domain {domain}"))
        })
    }

    pub fn try_chain_by_domain(&self, domain: u32) -> Option<ChainMetadata> {
        self.metadata_by_domain
            .get(&domain)
            .and_then(|name| self.metadata_by_name.get(name))
            .cloned()
    }

    pub fn chain_count(&self) -> usize {
        self.metadata_by_name.len()
    }
}

pub fn parse_chain_metadata(path: &Path, raw: &str) -> Result<ChainMetadata> {
    serde_yaml::from_str(raw).map_err(|source| IgpOracleError::Yaml {
        path: path.to_path_buf(),
        source,
    })
}

pub fn parse_core_addresses(path: &Path, raw: &str) -> Result<CoreAddresses> {
    serde_yaml::from_str(raw).map_err(|source| IgpOracleError::Yaml {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use crate::models::ChainProtocol;

    use super::*;

    #[test]
    fn parses_celestiatestnet_metadata_as_cosmosnative() {
        let metadata = parse_chain_metadata(
            Path::new("metadata.yaml"),
            include_str!("../../../chains/celestiatestnet/metadata.yaml"),
        )
        .expect("metadata should parse");

        assert_eq!(metadata.name, "celestiatestnet");
        assert_eq!(metadata.protocol, ChainProtocol::CosmosNative);
        assert_eq!(metadata.domain_id, 1_297_040_200);
        assert_eq!(metadata.native_token.symbol, "TIA");
    }

    #[test]
    fn parses_edentestnet_metadata_as_ethereum() {
        let metadata = parse_chain_metadata(
            Path::new("metadata.yaml"),
            include_str!("../../../chains/edentestnet/metadata.yaml"),
        )
        .expect("metadata should parse");

        assert_eq!(metadata.name, "edentestnet");
        assert_eq!(metadata.protocol, ChainProtocol::Ethereum);
        assert_eq!(metadata.domain_id, 2_147_483_647);
    }

    #[test]
    fn parses_ethereum_metadata() {
        let metadata = parse_chain_metadata(
            Path::new("metadata.yaml"),
            include_str!("../../../chains/ethereum/metadata.yaml"),
        )
        .expect("metadata should parse");

        assert_eq!(metadata.name, "ethereum");
        assert_eq!(metadata.protocol, ChainProtocol::Ethereum);
        assert_eq!(metadata.domain_id, 1);
        assert_eq!(metadata.native_token.symbol, "ETH");
    }

    #[test]
    fn parses_arbitrum_metadata() {
        let metadata = parse_chain_metadata(
            Path::new("metadata.yaml"),
            include_str!("../../../chains/arbitrum/metadata.yaml"),
        )
        .expect("metadata should parse");

        assert_eq!(metadata.name, "arbitrum");
        assert_eq!(metadata.protocol, ChainProtocol::Ethereum);
        assert_eq!(metadata.domain_id, 42_161);
        assert_eq!(metadata.native_token.symbol, "ETH");
    }

    #[test]
    fn parses_interchain_gas_paymaster_address() {
        let addresses = parse_core_addresses(
            Path::new("addresses.yaml"),
            include_str!("../../../chains/celestiatestnet/addresses.yaml"),
        )
        .expect("addresses should parse");

        assert_eq!(
            addresses.interchain_gas_paymaster.as_deref(),
            Some("0x726f757465725f706f73745f6469737061746368000000040000000000000003")
        );
    }

    #[test]
    fn registry_index_resolves_by_name_and_domain() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");

        let index = RegistryIndex::load(&repo_root).expect("registry should index");

        assert!(index.chain_count() >= 4);
        assert_eq!(
            index
                .chain_metadata("ethereum")
                .expect("ethereum metadata")
                .domain_id,
            1
        );
        assert_eq!(
            index
                .try_chain_by_domain(42_161)
                .expect("arbitrum domain")
                .name,
            "arbitrum"
        );
        assert_eq!(
            index
                .core_addresses("celestiatestnet")
                .expect("celestiatestnet addresses")
                .interchain_gas_paymaster
                .as_deref(),
            Some("0x726f757465725f706f73745f6469737061746368000000040000000000000003")
        );
    }

    #[test]
    fn registry_index_reports_unknown_chain() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let index = RegistryIndex::load(&repo_root).expect("registry should index");

        let err = index
            .chain_metadata("missing-chain")
            .expect_err("missing chain should fail");

        assert!(matches!(err, IgpOracleError::Registry(_)));
    }
}
