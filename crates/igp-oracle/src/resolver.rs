use crate::{
    cli::ReconcileArgs,
    config::{TargetConfig, UpdaterConfig},
    error::{IgpOracleError, Result},
    models::ReconciliationTarget,
    registry::RegistryLoader,
};

pub fn resolve_targets(
    config: &UpdaterConfig,
    registry: &RegistryLoader,
    args: &ReconcileArgs,
) -> Result<Vec<ReconciliationTarget>> {
    let mut targets = Vec::new();

    for target in config.targets.iter().filter(|target| target.enabled) {
        if !matches_origin(target, args) || !matches_remote(target, args) {
            continue;
        }

        validate_target(target)?;

        let origin = registry.load_chain_metadata(&target.origin_chain)?;
        let remote = match (&target.remote_chain, target.remote_domain) {
            (Some(chain), _) => registry.load_chain_metadata(chain)?,
            (None, Some(domain)) => registry.find_chain_by_domain(domain)?,
            (None, None) => {
                return Err(IgpOracleError::InvalidTarget(format!(
                    "target {} must define remoteChain or remoteDomain",
                    target.origin_chain
                )))
            }
        };
        let origin_addresses = registry.load_core_addresses(&target.origin_chain)?;

        targets.push(ReconciliationTarget {
            origin,
            remote,
            origin_addresses,
            config: target.clone(),
        });
    }

    if targets.is_empty() {
        return Err(IgpOracleError::InvalidTarget(
            "no enabled targets matched the supplied filters".to_string(),
        ));
    }

    Ok(targets)
}

fn matches_origin(target: &TargetConfig, args: &ReconcileArgs) -> bool {
    args.origin
        .as_ref()
        .is_none_or(|origin| origin == &target.origin_chain)
}

fn matches_remote(target: &TargetConfig, args: &ReconcileArgs) -> bool {
    let chain_matches = args
        .remote_chain
        .as_ref()
        .is_none_or(|remote| target.remote_chain.as_ref() == Some(remote));
    let domain_matches = args
        .remote_domain
        .is_none_or(|domain| target.remote_domain == Some(domain));

    chain_matches && domain_matches
}

fn validate_target(target: &TargetConfig) -> Result<()> {
    if target.remote_chain.is_none() && target.remote_domain.is_none() {
        return Err(IgpOracleError::InvalidTarget(format!(
            "target {} must define remoteChain or remoteDomain",
            target.origin_chain
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::{config::UpdaterConfig, registry::RegistryLoader};

    use super::*;

    #[test]
    fn resolves_single_target() {
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

        let targets = resolve_targets(&config, &registry, &args).expect("target should resolve");

        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].origin.name, "celestiatestnet");
        assert_eq!(targets[0].remote.name, "edentestnet");
    }
}
