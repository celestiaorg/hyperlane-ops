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
    let mut expanded = Vec::new();

    for configured in configs {
        if remote_domain_filter.is_some_and(|domain| domain != configured.remote_domain) {
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
}
