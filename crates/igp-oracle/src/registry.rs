use std::path::{Path, PathBuf};

use crate::{
    error::{read_to_string, IgpOracleError, Result},
    models::{ChainMetadata, CoreAddresses},
};

#[derive(Debug, Clone)]
pub struct RegistryLoader {
    root: PathBuf,
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
                return Ok(metadata);
            }
        }

        Err(IgpOracleError::Registry(format!(
            "no chain metadata found for domain {domain}"
        )))
    }

    fn chain_path(&self, chain: &str) -> PathBuf {
        self.root.join("chains").join(chain)
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
    use std::path::Path;

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
}
