use std::{path::Path, process::Command};

use crate::{
    adapters::{adapter_for, ChainAdapter, GasAdapter, PriceAdapter},
    artifacts::{
        target_artifact, write_artifacts, DecisionArtifact, DiscoveryArtifact, PlanArtifact,
        PolicyArtifact, SkippedTargetArtifact, TargetArtifactInput,
    },
    cli::ReconcileArgs,
    config::UpdaterConfig,
    data::{CoinGeckoPriceAdapter, ProtocolGasAdapter},
    error::{IgpOracleError, Result},
    models::ChainProtocol,
    policy::{compute_proposal, decide_reconciliation},
    registry::RegistryLoader,
    resolver::{expand_configured_domains, resolve_origin_work_items, ExpandedTarget},
};

type ChainAdapterFactory = dyn Fn(ChainProtocol) -> Box<dyn ChainAdapter> + Sync;

pub async fn run_reconcile(args: ReconcileArgs) -> Result<i32> {
    let config = UpdaterConfig::load(&args.config)?;
    let price_adapter = CoinGeckoPriceAdapter::new(&config.market_data)?;
    let gas_adapter = ProtocolGasAdapter::new();

    run_reconcile_with_sources(args, &config, &price_adapter, &gas_adapter).await
}

pub async fn run_reconcile_with_sources(
    args: ReconcileArgs,
    config: &UpdaterConfig,
    price_adapter: &dyn PriceAdapter,
    gas_adapter: &dyn GasAdapter,
) -> Result<i32> {
    run_reconcile_with_sources_and_adapter_factory(
        args,
        config,
        price_adapter,
        gas_adapter,
        &adapter_for,
    )
    .await
}

pub async fn run_reconcile_with_sources_and_adapter_factory(
    args: ReconcileArgs,
    config: &UpdaterConfig,
    price_adapter: &dyn PriceAdapter,
    gas_adapter: &dyn GasAdapter,
    adapter_factory: &ChainAdapterFactory,
) -> Result<i32> {
    if args.write {
        return Err(IgpOracleError::UnsupportedWrite);
    }

    let registry = RegistryLoader::new(&args.registry);
    let work_items = resolve_origin_work_items(config, &registry, &args)?;

    let mut target_artifacts = Vec::new();
    let mut skipped_targets = Vec::new();
    let mut discovery = Vec::new();

    for work_item in work_items {
        let adapter = adapter_factory(work_item.origin.protocol);
        let configured = adapter
            .list_igp_destination_configs(&work_item.origin, &work_item.origin_addresses)
            .await?;
        let configured_count = configured.len();
        let expanded = expand_configured_domains(&work_item, &registry, &args, configured)?;
        let mut resolved_count = 0usize;
        let mut skipped_count = 0usize;

        for expanded_target in expanded {
            match expanded_target {
                ExpandedTarget::Reconcile {
                    target,
                    current_read,
                } => {
                    resolved_count += 1;
                    let artifact = match reconcile_target(
                        *target,
                        current_read,
                        config,
                        price_adapter,
                        gas_adapter,
                        adapter.as_ref(),
                    )
                    .await
                    {
                        Ok(artifact) => artifact,
                        Err((target, current_read, err)) => error_target_artifact(
                            &target,
                            current_read,
                            config,
                            "data_source_error",
                            "data_source_error",
                            err.to_string(),
                        ),
                    };
                    target_artifacts.push(artifact);
                }
                ExpandedTarget::Skipped {
                    origin_chain,
                    remote_domain,
                    code,
                    reason,
                } => {
                    skipped_count += 1;
                    skipped_targets.push(SkippedTargetArtifact {
                        origin_chain,
                        remote_domain,
                        status: "skipped".to_string(),
                        code,
                        reason,
                    });
                }
            }
        }

        discovery.push(DiscoveryArtifact {
            origin_chain: work_item.origin.name.clone(),
            igp_identifier: work_item.origin_addresses.interchain_gas_paymaster.clone(),
            protocol: work_item.origin.protocol.as_str().to_string(),
            configured_remote_domains: configured_count,
            resolved_remote_domains: resolved_count,
            skipped_remote_domains: skipped_count,
        });
    }

    let plan = PlanArtifact {
        git_sha: git_sha(&args.registry),
        discovery,
        skipped_targets,
        targets: target_artifacts,
    };

    write_artifacts(&args.output_dir, &plan)?;
    Ok(exit_code_for_plan(&plan))
}

async fn reconcile_target(
    target: crate::models::ReconciliationTarget,
    current_read: crate::models::IgpConfigRead,
    config: &UpdaterConfig,
    price_adapter: &dyn PriceAdapter,
    gas_adapter: &dyn GasAdapter,
    adapter: &dyn ChainAdapter,
) -> std::result::Result<
    crate::artifacts::TargetPlanArtifact,
    (
        crate::models::ReconciliationTarget,
        crate::models::IgpConfigRead,
        IgpOracleError,
    ),
> {
    let proposal = compute_proposal(&target, &config.defaults, gas_adapter, price_adapter)
        .await
        .map_err(|err| (target.clone(), current_read.clone(), err))?;
    let reconciliation =
        decide_reconciliation(&current_read.config, &proposal.proposed, &config.defaults)
            .map_err(|err| (target.clone(), current_read.clone(), err))?;
    let deltas = Some(reconciliation.deltas);
    let decision = DecisionArtifact {
        status: reconciliation.status.as_str().to_string(),
        code: reconciliation.code.as_str().to_string(),
        field: reconciliation.field.map(|field| field.as_str().to_string()),
        max_delta_bps: reconciliation.max_delta_bps,
        reason: reconciliation.reason,
    };
    let tx = if decision.status != "noop" {
        match adapter.plan_update(&target, &proposal.proposed).await {
            Ok(plan) => Some(plan),
            Err(IgpOracleError::UnsupportedLiveRead(_)) => None,
            Err(err) => return Err((target, current_read, err)),
        }
    } else {
        None
    };

    Ok(target_artifact(TargetArtifactInput {
        target: &target,
        config,
        policy: PolicyArtifact::from(&config.defaults),
        proposal: Some(proposal),
        current_read: Some(current_read),
        deltas,
        tx,
        decision,
    }))
}

fn error_target_artifact(
    target: &crate::models::ReconciliationTarget,
    current_read: crate::models::IgpConfigRead,
    config: &UpdaterConfig,
    status: &str,
    code: &str,
    reason: String,
) -> crate::artifacts::TargetPlanArtifact {
    target_artifact(TargetArtifactInput {
        target,
        config,
        policy: PolicyArtifact::from(&config.defaults),
        proposal: None,
        current_read: Some(current_read),
        deltas: None,
        tx: None,
        decision: DecisionArtifact {
            status: status.to_string(),
            code: code.to_string(),
            field: None,
            max_delta_bps: None,
            reason,
        },
    })
}

fn exit_code_for_plan(plan: &PlanArtifact) -> i32 {
    if plan
        .targets
        .iter()
        .any(|target| target.decision.status == "policy_violation")
    {
        return 30;
    }

    if plan
        .targets
        .iter()
        .any(|target| target.decision.status == "data_source_error")
    {
        return 20;
    }

    if plan
        .targets
        .iter()
        .any(|target| target.decision.status == "update_recommended")
    {
        return 10;
    }

    0
}

fn git_sha(registry: &Path) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(registry)
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let sha = String::from_utf8(output.stdout).ok()?;
    let sha = sha.trim();
    if sha.is_empty() {
        None
    } else {
        Some(sha.to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, path::PathBuf};

    use async_trait::async_trait;
    use rust_decimal::Decimal;
    use tempfile::tempdir;

    use crate::{
        adapters::{ChainAdapter, GasAdapter, PriceAdapter},
        cli::ReconcileArgs,
        models::{
            ChainMetadata, ConfiguredRemoteDomain, CoreAddresses, CurrentIgpConfig, IgpConfigRead,
            OnChainReadSource, ProposedIgpConfig, ReconciliationTarget, TxPlan, TxReceipt,
            TxSigner, VerificationResult,
        },
    };

    use super::*;

    struct StaticGasAdapter(u128);

    #[async_trait]
    impl GasAdapter for StaticGasAdapter {
        async fn remote_gas_price(&self, _target: &ReconciliationTarget) -> Result<u128> {
            Ok(self.0)
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

    struct StaticChainAdapter {
        protocol: ChainProtocol,
        configs: Vec<ConfiguredRemoteDomain>,
    }

    #[async_trait]
    impl ChainAdapter for StaticChainAdapter {
        fn protocol(&self) -> ChainProtocol {
            self.protocol
        }

        async fn list_igp_destination_configs(
            &self,
            _origin: &ChainMetadata,
            _origin_addresses: &CoreAddresses,
        ) -> Result<Vec<ConfiguredRemoteDomain>> {
            Ok(self.configs.clone())
        }

        async fn read_igp_config(&self, target: &ReconciliationTarget) -> Result<IgpConfigRead> {
            let config = self
                .configs
                .iter()
                .find(|config| config.remote_domain == target.remote.domain_id)
                .map(|config| config.current.clone())
                .ok_or_else(|| {
                    IgpOracleError::UnsupportedLiveRead(format!(
                        "test adapter for origin {}",
                        target.origin.name
                    ))
                })?;
            Ok(IgpConfigRead {
                config,
                source: OnChainReadSource {
                    protocol: self.protocol.as_str().to_string(),
                    endpoint: Some("test://endpoint".to_string()),
                    query: "test-query".to_string(),
                },
            })
        }

        async fn plan_update(
            &self,
            target: &ReconciliationTarget,
            proposed: &ProposedIgpConfig,
        ) -> Result<TxPlan> {
            Ok(TxPlan {
                protocol: self.protocol.as_str().to_string(),
                action: "setDestinationGasConfig".to_string(),
                message_type: "/hyperlane.core.post_dispatch.v1.MsgSetDestinationGasConfig"
                    .to_string(),
                target: target
                    .origin_addresses
                    .interchain_gas_paymaster
                    .clone()
                    .unwrap_or_else(|| "test-igp".to_string()),
                selector: None,
                calldata: None,
                command: None,
                signer: Some(TxSigner {
                    signer_profile: target.config.write.signer_profile.clone(),
                    address: Some("test-owner".to_string()),
                }),
                message: serde_json::json!({
                    "owner": "test-owner",
                    "igpId": "test-igp",
                    "destinationGasConfig": {
                        "remoteDomain": target.remote.domain_id,
                        "gasOracle": {
                            "tokenExchangeRate": proposed.token_exchange_rate.as_str(),
                            "gasPrice": proposed.gas_price.as_str()
                        },
                        "gasOverhead": proposed.gas_overhead.to_string()
                    }
                }),
                notes: vec!["test tx plan".to_string()],
            })
        }

        async fn submit_update(
            &self,
            _target: &ReconciliationTarget,
            _plan: &TxPlan,
        ) -> Result<TxReceipt> {
            Err(IgpOracleError::UnsupportedWrite)
        }

        async fn verify_update(
            &self,
            _target: &ReconciliationTarget,
            _expected: &ProposedIgpConfig,
        ) -> Result<VerificationResult> {
            Err(IgpOracleError::UnsupportedLiveRead(
                "test verification".to_string(),
            ))
        }
    }

    #[tokio::test]
    async fn write_mode_is_rejected() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let args = ReconcileArgs {
            config: repo_root.join("crates/igp-oracle/igp-oracle.example.yaml"),
            registry: repo_root,
            origin: Some("celestiatestnet".to_string()),
            remote_chain: Some("edentestnet".to_string()),
            remote_domain: None,
            output_dir: PathBuf::from("artifacts"),
            format: "markdown,json".to_string(),
            dry_run: false,
            write: true,
        };

        let err = run_reconcile(args)
            .await
            .expect_err("write mode should fail");
        assert!(matches!(err, IgpOracleError::UnsupportedWrite));
        assert_eq!(err.exit_code(), 40);
    }

    #[tokio::test]
    async fn dry_run_writes_artifacts() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let output_dir = tempdir().expect("tempdir");
        let config =
            UpdaterConfig::load(&repo_root.join("crates/igp-oracle/igp-oracle.example.yaml"))
                .expect("config should load");
        let mut prices = BTreeMap::new();
        prices.insert("celestiatestnet".to_string(), Decimal::from(2));
        prices.insert("edentestnet".to_string(), Decimal::from(4));
        let args = ReconcileArgs {
            config: repo_root.join("crates/igp-oracle/igp-oracle.example.yaml"),
            registry: repo_root,
            origin: Some("celestiatestnet".to_string()),
            remote_chain: Some("edentestnet".to_string()),
            remote_domain: None,
            output_dir: output_dir.path().to_path_buf(),
            format: "markdown,json".to_string(),
            dry_run: true,
            write: false,
        };

        let current = CurrentIgpConfig {
            gas_price: "110".to_string(),
            token_exchange_rate: "1".to_string(),
            gas_overhead: 174_289,
        };
        let adapter_factory = move |protocol| {
            Box::new(StaticChainAdapter {
                protocol,
                configs: vec![configured_remote(2_147_483_647, current.clone())],
            }) as Box<dyn ChainAdapter>
        };

        let code = run_reconcile_with_sources_and_adapter_factory(
            args,
            &config,
            &StaticPriceAdapter(prices),
            &StaticGasAdapter(100),
            &adapter_factory,
        )
        .await
        .expect("dry-run should succeed");
        assert_eq!(code, 0);
        assert!(output_dir.path().join("igp-summary.md").exists());
        assert!(output_dir.path().join("igp-plan.json").exists());
        assert!(output_dir.path().join("tx-plan.json").exists());

        let plan = std::fs::read_to_string(output_dir.path().join("igp-plan.json")).expect("plan");
        assert!(plan.contains("\"status\": \"noop\""));
        assert!(plan.contains("\"code\": \"noop\""));
        assert!(plan.contains("\"inputs\": {"));
        assert!(plan.contains("\"onChainRead\": {"));
        assert!(plan.contains("\"current\": {"));
        assert!(plan.contains("\"deltas\": {"));
        assert!(plan.contains("\"gasPriceBps\": 0"));
        assert!(plan.contains("\"originNativeTokenDecimals\": 6"));
        assert!(plan.contains("\"remoteNativeTokenDecimals\": 18"));
        assert!(plan.contains("\"tokenDecimalAdjustment\": \"0.000000000001\""));
        assert!(plan.contains("\"gasPrice\": \"110\""));
        assert!(plan.contains("\"tokenExchangeRate\": \"1\""));
        assert!(plan.contains("\"tx\": null"));
    }

    #[tokio::test]
    async fn dry_run_returns_ten_when_update_is_recommended() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let output_dir = tempdir().expect("tempdir");
        let config =
            UpdaterConfig::load(&repo_root.join("crates/igp-oracle/igp-oracle.example.yaml"))
                .expect("config should load");
        let mut prices = BTreeMap::new();
        prices.insert("celestiatestnet".to_string(), Decimal::from(2));
        prices.insert("edentestnet".to_string(), Decimal::from(4));
        let args = ReconcileArgs {
            config: repo_root.join("crates/igp-oracle/igp-oracle.example.yaml"),
            registry: repo_root,
            origin: Some("celestiatestnet".to_string()),
            remote_chain: Some("edentestnet".to_string()),
            remote_domain: None,
            output_dir: output_dir.path().to_path_buf(),
            format: "markdown,json".to_string(),
            dry_run: true,
            write: false,
        };
        let current = CurrentIgpConfig {
            gas_price: "100".to_string(),
            token_exchange_rate: "1".to_string(),
            gas_overhead: 174_289,
        };
        let adapter_factory = move |protocol| {
            Box::new(StaticChainAdapter {
                protocol,
                configs: vec![configured_remote(2_147_483_647, current.clone())],
            }) as Box<dyn ChainAdapter>
        };

        let code = run_reconcile_with_sources_and_adapter_factory(
            args,
            &config,
            &StaticPriceAdapter(prices),
            &StaticGasAdapter(100),
            &adapter_factory,
        )
        .await
        .expect("dry-run should succeed");

        assert_eq!(code, 10);
        let plan = std::fs::read_to_string(output_dir.path().join("igp-plan.json")).expect("plan");
        assert!(plan.contains("\"status\": \"update_recommended\""));
        assert!(plan.contains("\"code\": \"update_threshold_met\""));
        assert!(plan.contains(
            "\"messageType\": \"/hyperlane.core.post_dispatch.v1.MsgSetDestinationGasConfig\""
        ));
        assert!(plan.contains("\"destinationGasConfig\""));
        let tx_plan =
            std::fs::read_to_string(output_dir.path().join("tx-plan.json")).expect("tx plan");
        assert!(tx_plan.contains("\"tx\": {"));
    }

    #[tokio::test]
    async fn dry_run_records_target_data_source_errors_and_continues() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let output_dir = tempdir().expect("tempdir");
        let config =
            UpdaterConfig::load(&repo_root.join("crates/igp-oracle/igp-oracle.example.yaml"))
                .expect("config should load");
        let mut prices = BTreeMap::new();
        prices.insert("celestiatestnet".to_string(), Decimal::from(2));
        prices.insert("edentestnet".to_string(), Decimal::from(4));
        let args = ReconcileArgs {
            config: repo_root.join("crates/igp-oracle/igp-oracle.example.yaml"),
            registry: repo_root,
            origin: Some("celestiatestnet".to_string()),
            remote_chain: None,
            remote_domain: None,
            output_dir: output_dir.path().to_path_buf(),
            format: "markdown,json".to_string(),
            dry_run: true,
            write: false,
        };
        let current = CurrentIgpConfig {
            gas_price: "100".to_string(),
            token_exchange_rate: "1".to_string(),
            gas_overhead: 174_289,
        };
        let adapter_factory = move |protocol| {
            Box::new(StaticChainAdapter {
                protocol,
                configs: vec![
                    configured_remote(2_147_483_647, current.clone()),
                    configured_remote(1_000_101, current.clone()),
                ],
            }) as Box<dyn ChainAdapter>
        };

        let code = run_reconcile_with_sources_and_adapter_factory(
            args,
            &config,
            &StaticPriceAdapter(prices),
            &StaticGasAdapter(100),
            &adapter_factory,
        )
        .await
        .expect("dry-run should write artifacts despite one bad target");

        assert_eq!(code, 20);
        let plan = std::fs::read_to_string(output_dir.path().join("igp-plan.json")).expect("plan");
        assert!(plan.contains("\"remoteChain\": \"edentestnet\""));
        assert!(plan.contains("\"remoteChain\": \"xomarkettestnet\""));
        assert!(plan.contains("\"status\": \"data_source_error\""));
        assert!(plan.contains("missing static price for xomarkettestnet"));
        assert!(plan.contains("\"status\": \"update_recommended\""));
    }

    fn configured_remote(remote_domain: u32, current: CurrentIgpConfig) -> ConfiguredRemoteDomain {
        ConfiguredRemoteDomain {
            remote_domain,
            current,
            source: OnChainReadSource {
                protocol: "cosmosnative".to_string(),
                endpoint: Some("test://endpoint".to_string()),
                query: "test-query".to_string(),
            },
        }
    }
}
