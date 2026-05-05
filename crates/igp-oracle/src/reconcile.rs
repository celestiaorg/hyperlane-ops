use std::{path::Path, process::Command};

use crate::{
    adapters::{adapter_for, ChainAdapter, GasAdapter, PriceAdapter},
    artifacts::{
        target_artifact, write_artifacts, DecisionArtifact, PlanArtifact, PolicyArtifact,
        TargetArtifactInput,
    },
    cli::ReconcileArgs,
    config::UpdaterConfig,
    data::{CoinGeckoPriceAdapter, ProtocolGasAdapter},
    error::{IgpOracleError, Result},
    models::ChainProtocol,
    policy::{compute_proposal, decide_reconciliation},
    registry::RegistryLoader,
    resolver::resolve_targets,
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
    let targets = resolve_targets(config, &registry, &args)?;

    let mut target_artifacts = Vec::new();
    for target in targets {
        let proposal =
            compute_proposal(&target, &config.defaults, gas_adapter, price_adapter).await?;
        let adapter = adapter_factory(target.origin.protocol);
        let (current_read, deltas, decision) = match adapter.read_igp_config(&target).await {
            Ok(read) => {
                let reconciliation =
                    decide_reconciliation(&read.config, &proposal.proposed, &config.defaults)?;
                (
                    Some(read),
                    Some(reconciliation.deltas),
                    DecisionArtifact {
                        status: reconciliation.status.as_str().to_string(),
                        code: reconciliation.code.as_str().to_string(),
                        field: reconciliation.field.map(|field| field.as_str().to_string()),
                        max_delta_bps: reconciliation.max_delta_bps,
                        reason: reconciliation.reason,
                    },
                )
            }
            Err(IgpOracleError::UnsupportedLiveRead(reason)) => (
                None,
                None,
                DecisionArtifact {
                    status: "unsupported_live_read".to_string(),
                    code: "unsupported_live_read".to_string(),
                    field: None,
                    max_delta_bps: None,
                    reason,
                },
            ),
            Err(err) => return Err(err),
        };
        let tx = if current_read.is_some() && decision.status != "noop" {
            match adapter.plan_update(&target, &proposal.proposed).await {
                Ok(plan) => Some(plan),
                Err(IgpOracleError::UnsupportedLiveRead(_)) => None,
                Err(err) => return Err(err),
            }
        } else {
            None
        };

        target_artifacts.push(target_artifact(TargetArtifactInput {
            target: &target,
            config,
            policy: PolicyArtifact::from(&config.defaults),
            proposal: Some(proposal),
            current_read,
            deltas,
            tx,
            decision,
        }));
    }

    let plan = PlanArtifact {
        git_sha: git_sha(&args.registry),
        targets: target_artifacts,
    };

    write_artifacts(&args.output_dir, &plan)?;
    Ok(exit_code_for_plan(&plan))
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
            CurrentIgpConfig, IgpConfigRead, OnChainReadSource, ProposedIgpConfig,
            ReconciliationTarget, TxPlan, TxReceipt, TxSigner, VerificationResult,
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
        current: Option<CurrentIgpConfig>,
    }

    #[async_trait]
    impl ChainAdapter for StaticChainAdapter {
        fn protocol(&self) -> ChainProtocol {
            self.protocol
        }

        async fn read_igp_config(&self, target: &ReconciliationTarget) -> Result<IgpConfigRead> {
            let config = self.current.clone().ok_or_else(|| {
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
            token_exchange_rate: "22000000000".to_string(),
            gas_overhead: 174_289,
        };
        let adapter_factory = move |protocol| {
            Box::new(StaticChainAdapter {
                protocol,
                current: Some(current.clone()),
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
        assert!(plan.contains("\"gasPrice\": \"110\""));
        assert!(plan.contains("\"tokenExchangeRate\": \"22000000000\""));
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
            token_exchange_rate: "22000000000".to_string(),
            gas_overhead: 174_289,
        };
        let adapter_factory = move |protocol| {
            Box::new(StaticChainAdapter {
                protocol,
                current: Some(current.clone()),
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
}
