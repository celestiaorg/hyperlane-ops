use crate::{
    cli::ReconcileArgs,
    config::{TargetConfig, UpdaterConfig},
    error::{IgpOracleError, Result},
    models::{
        ChainMetadata, ConfiguredRemoteDomain, CoreAddresses, IgpConfigRead, ReconciliationTarget,
    },
    registry::RegistryLoader,
};

#[derive(Debug, Clone)]
pub struct OriginWorkItem {
    pub origin: ChainMetadata,
    pub origin_addresses: CoreAddresses,
    pub config: TargetConfig,
}

#[derive(Debug, Clone)]
pub enum ExpandedTarget {
    Reconcile {
        target: Box<ReconciliationTarget>,
        current_read: IgpConfigRead,
    },
    Skipped {
        origin_chain: String,
        remote_domain: u32,
        code: String,
        reason: String,
    },
}

pub fn resolve_origin_work_items(
    config: &UpdaterConfig,
    registry: &RegistryLoader,
    args: &ReconcileArgs,
) -> Result<Vec<OriginWorkItem>> {
    let mut work_items = Vec::new();

    for target in config.targets.iter().filter(|target| target.enabled) {
        if !matches_origin(target, args) {
            continue;
        }

        let origin = registry.load_chain_metadata(&target.origin_chain)?;
        let origin_addresses = registry.load_core_addresses(&target.origin_chain)?;

        work_items.push(OriginWorkItem {
            origin,
            origin_addresses,
            config: target.clone(),
        });
    }

    if work_items.is_empty() {
        return Err(IgpOracleError::InvalidTarget(
            "no enabled origin targets matched the supplied filters".to_string(),
        ));
    }

    Ok(work_items)
}

pub fn expand_configured_domains(
    work_item: &OriginWorkItem,
    registry: &RegistryLoader,
    args: &ReconcileArgs,
    configs: Vec<ConfiguredRemoteDomain>,
) -> Result<Vec<ExpandedTarget>> {
    let remote_domain_filter = remote_domain_filter(registry, args)?;
    let is_unfiltered_sweep = remote_domain_filter.is_none();
    let mut expanded = Vec::new();

    for configured in configs {
        if remote_domain_filter.is_some_and(|domain| domain != configured.remote_domain) {
            continue;
        }

        if is_unfiltered_sweep && configured.remote_domain == work_item.origin.domain_id {
            expanded.push(ExpandedTarget::Skipped {
                origin_chain: work_item.origin.name.clone(),
                remote_domain: configured.remote_domain,
                code: "self_domain".to_string(),
                reason: format!(
                    "remote domain {} is the origin domain and is skipped during sweep mode",
                    configured.remote_domain
                ),
            });
            continue;
        }

        match registry.try_find_chain_by_domain(configured.remote_domain)? {
            Some(remote) => {
                let current_read = IgpConfigRead {
                    config: configured.current.clone(),
                    source: configured.source.clone(),
                };
                expanded.push(ExpandedTarget::Reconcile {
                    target: Box::new(ReconciliationTarget {
                        origin: work_item.origin.clone(),
                        remote,
                        origin_addresses: work_item.origin_addresses.clone(),
                        config: work_item.config.clone(),
                        gas_overhead: configured.current.gas_overhead,
                    }),
                    current_read,
                });
            }
            None => expanded.push(ExpandedTarget::Skipped {
                origin_chain: work_item.origin.name.clone(),
                remote_domain: configured.remote_domain,
                code: "missing_registry_metadata".to_string(),
                reason: format!(
                    "no local chain metadata for remote domain {}",
                    configured.remote_domain
                ),
            }),
        }
    }

    Ok(expanded)
}

fn matches_origin(target: &TargetConfig, args: &ReconcileArgs) -> bool {
    args.origin
        .as_ref()
        .is_none_or(|origin| origin == &target.origin_chain)
}

fn remote_domain_filter(registry: &RegistryLoader, args: &ReconcileArgs) -> Result<Option<u32>> {
    if let Some(domain) = args.remote_domain {
        return Ok(Some(domain));
    }

    args.remote_chain
        .as_ref()
        .map(|chain| {
            registry
                .load_chain_metadata(chain)
                .map(|metadata| metadata.domain_id)
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde::Deserialize;

    use crate::{
        config::UpdaterConfig,
        models::{CurrentIgpConfig, OnChainReadSource},
        registry::RegistryLoader,
    };

    use super::*;

    #[test]
    fn resolves_origin_work_item() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let config =
            UpdaterConfig::load(&repo_root.join("crates/igp-oracle/igp-oracle.example.yaml"))
                .expect("config should load");
        let registry = RegistryLoader::new(&repo_root);
        let args = ReconcileArgs {
            config: repo_root.join("crates/igp-oracle/igp-oracle.example.yaml"),
            registry: repo_root,
            origin: Some("celestiatestnet".to_string()),
            remote_chain: Some("edentestnet".to_string()),
            remote_domain: None,
            output_dir: PathBuf::from("artifacts"),
            format: "markdown,json".to_string(),
            dry_run: true,
            write: false,
        };

        let work_items =
            resolve_origin_work_items(&config, &registry, &args).expect("work item should resolve");

        assert_eq!(work_items.len(), 1);
        assert_eq!(work_items[0].origin.name, "celestiatestnet");
    }

    #[test]
    fn expands_known_domains_and_records_unknown_domain_skips() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let config =
            UpdaterConfig::load(&repo_root.join("crates/igp-oracle/igp-oracle.example.yaml"))
                .expect("config should load");
        let registry = RegistryLoader::new(&repo_root);
        let args = ReconcileArgs {
            config: repo_root.join("crates/igp-oracle/igp-oracle.example.yaml"),
            registry: repo_root,
            origin: Some("celestiatestnet".to_string()),
            remote_chain: None,
            remote_domain: None,
            output_dir: PathBuf::from("artifacts"),
            format: "markdown,json".to_string(),
            dry_run: true,
            write: false,
        };
        let work_item = resolve_origin_work_items(&config, &registry, &args)
            .expect("work item should resolve")
            .remove(0);
        let source = OnChainReadSource {
            protocol: "cosmosnative".to_string(),
            endpoint: Some("test://grpc".to_string()),
            query: "test-query".to_string(),
        };
        let configured = vec![
            ConfiguredRemoteDomain {
                remote_domain: 2_147_483_647,
                current: CurrentIgpConfig {
                    gas_price: "100".to_string(),
                    token_exchange_rate: "1".to_string(),
                    gas_overhead: 300_000,
                },
                source: source.clone(),
            },
            ConfiguredRemoteDomain {
                remote_domain: 123_456,
                current: CurrentIgpConfig {
                    gas_price: "100".to_string(),
                    token_exchange_rate: "1".to_string(),
                    gas_overhead: 174_289,
                },
                source,
            },
        ];

        let expanded = expand_configured_domains(&work_item, &registry, &args, configured)
            .expect("domains should expand");

        assert_eq!(expanded.len(), 2);
        assert!(matches!(
            &expanded[0],
            ExpandedTarget::Reconcile { target, .. } if target.remote.name == "edentestnet" && target.gas_overhead == 300_000
        ));
        assert!(matches!(
            &expanded[1],
            ExpandedTarget::Skipped { remote_domain: 123_456, code, .. } if code == "missing_registry_metadata"
        ));
    }

    #[test]
    fn sweep_skips_origin_domain_without_remote_filter() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let config =
            UpdaterConfig::load(&repo_root.join("crates/igp-oracle/igp-oracle.example.yaml"))
                .expect("config should load");
        let registry = RegistryLoader::new(&repo_root);
        let args = ReconcileArgs {
            config: repo_root.join("crates/igp-oracle/igp-oracle.example.yaml"),
            registry: repo_root,
            origin: Some("celestiatestnet".to_string()),
            remote_chain: None,
            remote_domain: None,
            output_dir: PathBuf::from("artifacts"),
            format: "markdown,json".to_string(),
            dry_run: true,
            write: false,
        };
        let work_item = resolve_origin_work_items(&config, &registry, &args)
            .expect("work item should resolve")
            .remove(0);
        let configured = vec![configured_remote(work_item.origin.domain_id)];

        let expanded = expand_configured_domains(&work_item, &registry, &args, configured)
            .expect("domains should expand");

        assert_eq!(expanded.len(), 1);
        assert!(matches!(
            &expanded[0],
            ExpandedTarget::Skipped { remote_domain, code, .. }
                if *remote_domain == 1_297_040_200 && code == "self_domain"
        ));
    }

    #[test]
    fn explicit_remote_filter_can_resolve_origin_domain() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let config =
            UpdaterConfig::load(&repo_root.join("crates/igp-oracle/igp-oracle.example.yaml"))
                .expect("config should load");
        let registry = RegistryLoader::new(&repo_root);
        let args = ReconcileArgs {
            config: repo_root.join("crates/igp-oracle/igp-oracle.example.yaml"),
            registry: repo_root,
            origin: Some("celestiatestnet".to_string()),
            remote_chain: Some("celestiatestnet".to_string()),
            remote_domain: None,
            output_dir: PathBuf::from("artifacts"),
            format: "markdown,json".to_string(),
            dry_run: true,
            write: false,
        };
        let work_item = resolve_origin_work_items(&config, &registry, &args)
            .expect("work item should resolve")
            .remove(0);
        let configured = vec![configured_remote(work_item.origin.domain_id)];

        let expanded = expand_configured_domains(&work_item, &registry, &args, configured)
            .expect("domains should expand");

        assert_eq!(expanded.len(), 1);
        assert!(matches!(
            &expanded[0],
            ExpandedTarget::Reconcile { target, .. } if target.remote.name == "celestiatestnet"
        ));
    }

    #[test]
    fn expands_celestia_mainnet_sample_sweep_offline() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut config =
            UpdaterConfig::load(&repo_root.join("crates/igp-oracle/igp-oracle.example.yaml"))
                .expect("config should load");
        config.targets[0].origin_chain = "celestia".to_string();
        let registry = RegistryLoader::new(&repo_root);
        let args = ReconcileArgs {
            config: repo_root.join("crates/igp-oracle/igp-oracle.example.yaml"),
            registry: repo_root,
            origin: Some("celestia".to_string()),
            remote_chain: None,
            remote_domain: None,
            output_dir: PathBuf::from("artifacts"),
            format: "markdown,json".to_string(),
            dry_run: true,
            write: false,
        };
        let work_item = resolve_origin_work_items(&config, &registry, &args)
            .expect("work item should resolve")
            .remove(0);
        let configured = celestia_sample_configs();

        let expanded = expand_configured_domains(&work_item, &registry, &args, configured)
            .expect("domains should expand");

        let resolved = expanded
            .iter()
            .filter_map(|target| match target {
                ExpandedTarget::Reconcile {
                    target,
                    current_read,
                } => Some((
                    target.remote.domain_id,
                    target.remote.name.as_str(),
                    current_read.config.gas_price.as_str(),
                    current_read.config.token_exchange_rate.as_str(),
                    current_read.config.gas_overhead,
                )),
                ExpandedTarget::Skipped { .. } => None,
            })
            .collect::<Vec<_>>();
        let skipped = expanded
            .iter()
            .filter_map(|target| match target {
                ExpandedTarget::Skipped {
                    remote_domain,
                    code,
                    ..
                } => Some((*remote_domain, code.as_str())),
                ExpandedTarget::Reconcile { .. } => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(expanded.len(), 141);
        assert!(resolved.contains(&(1, "ethereum", "300000000", "101", 174_289)));
        assert!(resolved.contains(&(42_161, "arbitrum", "204331055", "101", 174_289)));
        assert!(skipped.contains(&(1_128_614_981, "self_domain")));
        assert!(skipped.contains(&(10, "missing_registry_metadata")));
    }

    fn configured_remote(remote_domain: u32) -> ConfiguredRemoteDomain {
        ConfiguredRemoteDomain {
            remote_domain,
            current: CurrentIgpConfig {
                gas_price: "100".to_string(),
                token_exchange_rate: "1".to_string(),
                gas_overhead: 300_000,
            },
            source: OnChainReadSource {
                protocol: "cosmosnative".to_string(),
                endpoint: Some("test://grpc".to_string()),
                query: "test-query".to_string(),
            },
        }
    }

    fn celestia_sample_configs() -> Vec<ConfiguredRemoteDomain> {
        let sample: SampleDestinationGasConfigs =
            serde_json::from_str(include_str!("../sample-configs.celestia.json"))
                .expect("sample configs should parse");
        let source = OnChainReadSource {
            protocol: "cosmosnative".to_string(),
            endpoint: Some("fixture://sample-configs.celestia.json".to_string()),
            query: "fixture".to_string(),
        };

        sample
            .destination_gas_configs
            .into_iter()
            .map(|config| ConfiguredRemoteDomain {
                remote_domain: config.remote_domain,
                current: CurrentIgpConfig {
                    gas_price: config.gas_oracle.gas_price,
                    token_exchange_rate: config.gas_oracle.token_exchange_rate,
                    gas_overhead: config
                        .gas_overhead
                        .parse()
                        .expect("sample gas overhead should parse"),
                },
                source: source.clone(),
            })
            .collect()
    }

    #[derive(Debug, Deserialize)]
    struct SampleDestinationGasConfigs {
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
}
