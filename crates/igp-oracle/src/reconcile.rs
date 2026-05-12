use std::{collections::BTreeSet, path::Path, process::Command};

use crate::{
    adapter::{ChainAdapter, ChainAdapterFactory, GasAdapter, PriceAdapter},
    adapter_for,
    artifacts::{
        target_artifact, write_artifacts, DecisionArtifact, DiscoveryArtifact, PlanArtifact,
        PolicyArtifact, SkippedTargetArtifact, TargetArtifactInput, TxPlanErrorArtifact,
        WritePlanMode,
    },
    cli::ReconcileArgs,
    config::UpdaterConfig,
    data::{CoinGeckoPriceAdapter, ProtocolGasAdapter},
    error::{IgpOracleError, Result},
    models::{ChainProtocol, ReconciliationTarget, TxPlan},
    plan::{build_write_plan, mark_write_plan_failed, submit_write_plan, ExecutableUpdate},
    policy::{compute_proposal, decide_reconciliation, DecisionStatus},
    registry::RegistryIndex,
    resolver::{
        expand_configured_domains, resolve_origin_work_items, selected_remote_domain_filter,
        ExpandedTarget, OriginWorkItem,
    },
};

pub async fn run_reconcile(args: ReconcileArgs) -> Result<i32> {
    validate_mode(&args)?;

    let config = UpdaterConfig::load(&args.config)?;
    let gas_adapter = ProtocolGasAdapter::new();
    let prepared = prepare_reconciliation(&args, &config, &adapter_for).await?;
    let price_adapter =
        CoinGeckoPriceAdapter::new_scoped(&config.market_data, required_price_chains(&prepared))?;

    reconcile_prepared(
        &args,
        &config,
        &price_adapter,
        &gas_adapter,
        prepared,
        &adapter_for,
    )
    .await
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
    validate_mode(&args)?;

    let prepared = prepare_reconciliation(&args, config, adapter_factory).await?;
    reconcile_prepared(
        &args,
        config,
        price_adapter,
        gas_adapter,
        prepared,
        adapter_factory,
    )
    .await
}

struct PreparedOrigin {
    origin_chain: String,
    igp_identifier: Option<String>,
    protocol: ChainProtocol,
    configured_count: usize,
    adapter: Box<dyn ChainAdapter>,
    expanded: Vec<ExpandedTarget>,
}

struct ReconciledTarget {
    artifact: crate::artifacts::TargetPlanArtifact,
    target: ReconciliationTarget,
    tx: Option<TxPlan>,
}

async fn prepare_reconciliation(
    args: &ReconcileArgs,
    config: &UpdaterConfig,
    adapter_factory: &ChainAdapterFactory,
) -> Result<Vec<PreparedOrigin>> {
    let registry = RegistryIndex::load(&args.registry)?;
    let work_items = resolve_origin_work_items(config, &registry, args)?;
    let mut prepared = Vec::new();

    for work_item in work_items {
        let adapter = adapter_factory(work_item.origin.protocol);
        let (configured_count, expanded) =
            prepare_origin_targets(&work_item, &registry, args, adapter.as_ref()).await?;

        prepared.push(PreparedOrigin {
            origin_chain: work_item.origin.name.clone(),
            igp_identifier: work_item.origin_addresses.interchain_gas_paymaster.clone(),
            protocol: work_item.origin.protocol,
            configured_count,
            adapter,
            expanded,
        });
    }

    Ok(prepared)
}

async fn prepare_origin_targets(
    work_item: &OriginWorkItem,
    registry: &RegistryIndex,
    args: &ReconcileArgs,
    adapter: &dyn ChainAdapter,
) -> Result<(usize, Vec<ExpandedTarget>)> {
    if let Some(domains) = work_item.config.remote_selection.operator_domains() {
        return expand_operator_domains(work_item, registry, args, adapter, domains).await;
    }

    if adapter.supports_destination_config_discovery() {
        let configured = adapter
            .list_igp_destination_configs(&work_item.origin, &work_item.origin_addresses)
            .await?;
        let configured_count = configured.len();
        let expanded = expand_configured_domains(work_item, registry, args, configured)?;
        return Ok((configured_count, expanded));
    }

    let Some(domain) = selected_remote_domain_filter(registry, args)? else {
        return Err(IgpOracleError::InvalidTarget(format!(
            "origin chain {} uses {} IGP contracts, which cannot enumerate configured remote domains; configure remoteSelection.domains or pass --remote-chain/--remote-domain",
            work_item.origin.name,
            adapter.protocol().as_str()
        )));
    };

    expand_operator_domains(work_item, registry, args, adapter, &[domain]).await
}

async fn expand_operator_domains(
    work_item: &OriginWorkItem,
    registry: &RegistryIndex,
    args: &ReconcileArgs,
    adapter: &dyn ChainAdapter,
    domains: &[u32],
) -> Result<(usize, Vec<ExpandedTarget>)> {
    let remote_domain_filter = selected_remote_domain_filter(registry, args)?;
    let is_unfiltered = remote_domain_filter.is_none();
    let mut expanded = Vec::new();

    for remote_domain in domains {
        if remote_domain_filter.is_some_and(|domain| domain != *remote_domain) {
            continue;
        }

        if is_unfiltered && *remote_domain == work_item.origin.domain_id {
            expanded.push(ExpandedTarget::Skipped {
                origin_chain: work_item.origin.name.clone(),
                remote_domain: *remote_domain,
                code: "self_domain".to_string(),
                reason: format!(
                    "remote domain {remote_domain} is the origin domain and is skipped during sweep mode"
                ),
            });
            continue;
        }

        let Some(remote) = registry.try_chain_by_domain(*remote_domain) else {
            expanded.push(ExpandedTarget::Skipped {
                origin_chain: work_item.origin.name.clone(),
                remote_domain: *remote_domain,
                code: "missing_registry_metadata".to_string(),
                reason: format!("no local chain metadata for remote domain {remote_domain}"),
            });
            continue;
        };

        let target = ReconciliationTarget {
            origin: work_item.origin.clone(),
            remote,
            origin_addresses: work_item.origin_addresses.clone(),
            config: work_item.config.clone(),
            gas_overhead: 0,
        };

        match adapter.read_igp_config(&target).await {
            Ok(current_read) => {
                let mut target = target;
                target.gas_overhead = current_read.config.gas_overhead;
                expanded.push(ExpandedTarget::Reconcile {
                    target: Box::new(target),
                    current_read,
                });
            }
            Err(err) => {
                expanded.push(ExpandedTarget::ReadError {
                    target: Box::new(target),
                    status: err.classify(),
                    reason: err.to_string(),
                });
            }
        }
    }

    Ok((domains.len(), expanded))
}

async fn reconcile_prepared(
    args: &ReconcileArgs,
    config: &UpdaterConfig,
    price_adapter: &dyn PriceAdapter,
    gas_adapter: &dyn GasAdapter,
    prepared: Vec<PreparedOrigin>,
    adapter_factory: &ChainAdapterFactory,
) -> Result<i32> {
    let mut target_artifacts = Vec::new();
    let mut skipped_targets = Vec::new();
    let mut discovery = Vec::new();
    let mut executable_updates = Vec::new();

    for prepared_origin in prepared {
        let mut resolved_count = 0usize;
        let mut skipped_count = 0usize;

        for expanded_target in prepared_origin.expanded {
            match expanded_target {
                ExpandedTarget::Reconcile {
                    target,
                    current_read,
                } => {
                    resolved_count += 1;
                    let reconciled = match reconcile_target(
                        *target,
                        current_read,
                        config,
                        price_adapter,
                        gas_adapter,
                        prepared_origin.adapter.as_ref(),
                    )
                    .await
                    {
                        Ok(reconciled) => reconciled,
                        Err((target, current_read, err)) => {
                            let artifact =
                                error_target_artifact(&target, current_read, config, &err);
                            target_artifacts.push(artifact);
                            continue;
                        }
                    };
                    if reconciled.artifact.decision.status == DecisionStatus::UpdateRecommended {
                        if let Some(tx) = reconciled.tx.clone() {
                            executable_updates.push(ExecutableUpdate {
                                target: reconciled.target.clone(),
                                tx,
                            });
                        }
                    }
                    target_artifacts.push(reconciled.artifact);
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
                        code,
                        reason,
                    });
                }
                ExpandedTarget::ReadError {
                    target,
                    status,
                    reason,
                } => {
                    resolved_count += 1;
                    target_artifacts
                        .push(target_read_error_artifact(&target, config, status, reason));
                }
            }
        }

        discovery.push(DiscoveryArtifact {
            origin_chain: prepared_origin.origin_chain,
            igp_identifier: prepared_origin.igp_identifier,
            protocol: prepared_origin.protocol,
            configured_remote_domains: prepared_origin.configured_count,
            resolved_remote_domains: resolved_count,
            skipped_remote_domains: skipped_count,
        });
    }

    let plan = PlanArtifact {
        git_sha: git_sha(&args.registry),
        policy: PolicyArtifact::from(&config.defaults),
        write_plan: None,
        discovery,
        skipped_targets,
        targets: target_artifacts,
    };

    let mut plan = plan;
    if args.write {
        let mode = if args.generate_only {
            WritePlanMode::GenerateOnly
        } else {
            WritePlanMode::Submit
        };
        plan.write_plan = Some(build_write_plan(config, &plan, mode, adapter_factory)?);
        if !args.generate_only {
            if let Err(err) =
                submit_write_plan(config, &mut plan, executable_updates, adapter_factory).await
            {
                mark_write_plan_failed(&mut plan, &err);
                write_artifacts(&args.output_dir, &plan)?;
                return Err(err);
            }
        }
    }

    write_artifacts(&args.output_dir, &plan)?;
    if args.write {
        Ok(0)
    } else {
        Ok(exit_code_for_plan(&plan))
    }
}

fn validate_mode(args: &ReconcileArgs) -> Result<()> {
    if args.write && args.dry_run {
        return Err(IgpOracleError::InvalidTarget(
            "--write and --dry-run are mutually exclusive".to_string(),
        ));
    }

    if args.generate_only && !args.write {
        return Err(IgpOracleError::InvalidTarget(
            "--generate-only requires --write".to_string(),
        ));
    }

    Ok(())
}

fn required_price_chains(prepared: &[PreparedOrigin]) -> BTreeSet<String> {
    prepared
        .iter()
        .flat_map(|origin| origin.expanded.iter())
        .filter_map(|expanded| match expanded {
            ExpandedTarget::Reconcile { target, .. } => {
                Some([target.origin.name.clone(), target.remote.name.clone()])
            }
            ExpandedTarget::ReadError { .. } | ExpandedTarget::Skipped { .. } => None,
        })
        .flatten()
        .collect()
}

async fn reconcile_target(
    target: crate::models::ReconciliationTarget,
    current_read: crate::models::IgpConfigRead,
    config: &UpdaterConfig,
    price_adapter: &dyn PriceAdapter,
    gas_adapter: &dyn GasAdapter,
    adapter: &dyn ChainAdapter,
) -> std::result::Result<
    ReconciledTarget,
    (
        crate::models::ReconciliationTarget,
        crate::models::IgpConfigRead,
        IgpOracleError,
    ),
> {
    let proposal = compute_proposal(
        &target,
        &current_read.config,
        &config.defaults,
        gas_adapter,
        price_adapter,
    )
    .await
    .map_err(|err| (target.clone(), current_read.clone(), err))?;
    let reconciliation =
        decide_reconciliation(&current_read.config, &proposal.proposed, &config.defaults)
            .map_err(|err| (target.clone(), current_read.clone(), err))?;
    let deltas = Some(reconciliation.deltas);
    let decision = DecisionArtifact {
        status: reconciliation.status,
        code: Some(reconciliation.code),
        field: reconciliation.field,
        delta_bps: reconciliation.observed_delta_bps,
        reason: reconciliation.reason,
    };
    let (tx, tx_plan_error) = if decision.status != DecisionStatus::Noop {
        match adapter.plan_update(&target, &proposal.proposed).await {
            Ok(plan) => (Some(plan), None),
            Err(err) => (
                None,
                Some(TxPlanErrorArtifact {
                    status: err.classify(),
                    reason: err.to_string(),
                }),
            ),
        }
    } else {
        (None, None)
    };

    let artifact = target_artifact(TargetArtifactInput {
        target: &target,
        config,
        proposal: Some(proposal),
        current_read: Some(current_read),
        deltas,
        tx: tx.clone(),
        tx_plan_error,
        decision,
    });

    Ok(ReconciledTarget {
        artifact,
        target,
        tx,
    })
}

fn error_target_artifact(
    target: &crate::models::ReconciliationTarget,
    current_read: crate::models::IgpConfigRead,
    config: &UpdaterConfig,
    err: &IgpOracleError,
) -> crate::artifacts::TargetPlanArtifact {
    target_artifact(TargetArtifactInput {
        target,
        config,
        proposal: None,
        current_read: Some(current_read),
        deltas: None,
        tx: None,
        tx_plan_error: None,
        decision: DecisionArtifact {
            status: err.classify(),
            code: None,
            field: None,
            delta_bps: None,
            reason: err.to_string(),
        },
    })
}

fn target_read_error_artifact(
    target: &crate::models::ReconciliationTarget,
    config: &UpdaterConfig,
    status: DecisionStatus,
    reason: String,
) -> crate::artifacts::TargetPlanArtifact {
    target_artifact(TargetArtifactInput {
        target,
        config,
        proposal: None,
        current_read: None,
        deltas: None,
        tx: None,
        tx_plan_error: None,
        decision: DecisionArtifact {
            status,
            code: None,
            field: None,
            delta_bps: None,
            reason,
        },
    })
}

fn exit_code_for_plan(plan: &PlanArtifact) -> i32 {
    if plan.targets.iter().any(|target| {
        matches!(
            target.decision.status,
            DecisionStatus::PolicyViolation | DecisionStatus::PolicyError
        )
    }) {
        return 30;
    }

    if plan
        .targets
        .iter()
        .any(|target| target.decision.status.is_error())
    {
        return 20;
    }

    if plan
        .targets
        .iter()
        .any(|target| target.decision.status == DecisionStatus::UpdateRecommended)
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
        adapter::{ChainAdapter, GasAdapter, PriceAdapter, SignerAuthStatus},
        cli::ReconcileArgs,
        config::{RemoteSelection, SignerConfig, WriteMethod},
        models::{
            ChainMetadata, ConfiguredRemoteDomain, CoreAddresses, GasPriceSample, IgpConfig,
            IgpConfigRead, OnChainReadSource, ReconciliationTarget, TxPayload, TxPlan, TxReceipt,
            TxSigner, VerificationResult,
        },
        proto::hyperlane::core::post_dispatch::v1::{
            DestinationGasConfig, GasOracle, MsgSetDestinationGasConfig,
        },
    };

    use super::*;

    struct StaticGasAdapter(u128);

    #[async_trait]
    impl GasAdapter for StaticGasAdapter {
        async fn remote_gas_price(&self, _target: &ReconciliationTarget) -> Result<GasPriceSample> {
            Ok(GasPriceSample {
                source: "test".to_string(),
                raw_amount: Some(self.0.to_string()),
                raw_denom: None,
                sampled_gas_price: self.0.to_string(),
                rounding: None,
                reason: None,
                endpoint: None,
            })
        }
    }

    struct StaticPriceAdapter(BTreeMap<String, Decimal>);

    #[async_trait]
    impl PriceAdapter for StaticPriceAdapter {
        async fn native_token_price_usd(&self, chain_name: &str) -> Result<Decimal> {
            self.0.get(chain_name).copied().ok_or_else(|| {
                IgpOracleError::MarketData(format!("missing static price for {chain_name}"))
            })
        }
    }

    struct StaticChainAdapter {
        protocol: ChainProtocol,
        configs: Vec<ConfiguredRemoteDomain>,
        fail_plan_update: bool,
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
                    protocol: self.protocol,
                    endpoint: Some("test://endpoint".to_string()),
                    query: "test-query".to_string(),
                },
            })
        }

        async fn plan_update(
            &self,
            target: &ReconciliationTarget,
            proposed: &IgpConfig,
        ) -> Result<TxPlan> {
            if self.fail_plan_update {
                return Err(IgpOracleError::OnchainRead(
                    "test tx plan source failed".to_string(),
                ));
            }

            let igp_id = target
                .origin_addresses
                .interchain_gas_paymaster
                .clone()
                .unwrap_or_else(|| "test-igp".to_string());
            let proto = MsgSetDestinationGasConfig {
                owner: "test-owner".to_string(),
                igp_id: igp_id.clone(),
                destination_gas_config: Some(DestinationGasConfig {
                    remote_domain: target.remote.domain_id,
                    gas_oracle: Some(GasOracle {
                        token_exchange_rate: proposed.token_exchange_rate.clone(),
                        gas_price: proposed.gas_price.clone(),
                    }),
                    gas_overhead: proposed.gas_overhead.to_string(),
                }),
            };
            Ok(TxPlan {
                protocol: self.protocol,
                action: "setDestinationGasConfig".to_string(),
                message_type: "/hyperlane.core.post_dispatch.v1.MsgSetDestinationGasConfig"
                    .to_string(),
                target: igp_id,
                selector: None,
                calldata: None,
                signer: Some(TxSigner {
                    signer_profile: target.config.write.signer_profile.clone(),
                    address: Some("test-owner".to_string()),
                }),
                payload: TxPayload::CosmosSetDestinationGasConfig(proto),
            })
        }

        async fn submit_update(
            &self,
            _target: &ReconciliationTarget,
            _plan: &TxPlan,
            _signer: &SignerConfig,
        ) -> Result<TxReceipt> {
            Ok(TxReceipt {
                tx_hash: "test-tx-hash".to_string(),
                height: Some(123),
            })
        }

        async fn verify_update(
            &self,
            _target: &ReconciliationTarget,
            _expected: &IgpConfig,
        ) -> Result<VerificationResult> {
            Err(IgpOracleError::UnsupportedLiveRead(
                "test verification".to_string(),
            ))
        }

        fn check_signer_authorization(
            &self,
            tx_signer: &TxSigner,
            signer_config: &SignerConfig,
        ) -> Result<SignerAuthStatus> {
            let Some(authorized) = tx_signer.address.as_deref() else {
                return Ok(SignerAuthStatus::AuthorityUnavailable);
            };
            // Mirrors cosmosnative adapter: short configured-signer values are treated as
            // key aliases (no address to compare).
            if signer_config.from.len() <= 20 || !signer_config.from.contains('1') {
                return Ok(SignerAuthStatus::KeyAliasUnverified);
            }
            if signer_config.from != authorized {
                return Err(IgpOracleError::InvalidTarget(format!(
                    "configured signer {} is not authorized; expected {authorized}",
                    signer_config.from
                )));
            }
            Ok(SignerAuthStatus::AddressMatch)
        }
    }

    struct NoDiscoveryChainAdapter {
        protocol: ChainProtocol,
        configs: Vec<ConfiguredRemoteDomain>,
    }

    #[async_trait]
    impl ChainAdapter for NoDiscoveryChainAdapter {
        fn protocol(&self) -> ChainProtocol {
            self.protocol
        }

        fn supports_destination_config_discovery(&self) -> bool {
            false
        }

        async fn list_igp_destination_configs(
            &self,
            origin: &ChainMetadata,
            _origin_addresses: &CoreAddresses,
        ) -> Result<Vec<ConfiguredRemoteDomain>> {
            Err(IgpOracleError::UnsupportedLiveRead(format!(
                "test adapter cannot enumerate origin {}",
                origin.name
            )))
        }

        async fn read_igp_config(&self, target: &ReconciliationTarget) -> Result<IgpConfigRead> {
            let config = self
                .configs
                .iter()
                .find(|config| config.remote_domain == target.remote.domain_id)
                .map(|config| config.current.clone())
                .ok_or_else(|| {
                    IgpOracleError::OnchainRead(format!(
                        "test adapter has no gas oracle configured for remote domain {}",
                        target.remote.domain_id
                    ))
                })?;

            Ok(IgpConfigRead {
                config,
                source: OnChainReadSource {
                    protocol: self.protocol,
                    endpoint: Some("test://evm-rpc".to_string()),
                    query: "test-direct-read".to_string(),
                },
            })
        }

        async fn plan_update(
            &self,
            target: &ReconciliationTarget,
            proposed: &IgpConfig,
        ) -> Result<TxPlan> {
            let gas_oracle = "0x1111111111111111111111111111111111111111".to_string();
            let token_exchange_rate = proposed.token_exchange_rate.parse::<u128>().unwrap_or(0);
            let gas_price = proposed.gas_price.parse::<u128>().unwrap_or(0);
            Ok(TxPlan {
                protocol: self.protocol,
                action: "setRemoteGasData".to_string(),
                message_type: "setRemoteGasData((uint32,uint128,uint128))".to_string(),
                target: gas_oracle.clone(),
                selector: Some("0xf3a1495f".to_string()),
                calldata: Some(format!("0xtest{:x}", target.remote.domain_id)),
                signer: Some(TxSigner {
                    signer_profile: target.config.write.signer_profile.clone(),
                    address: Some("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()),
                }),
                payload: TxPayload::EvmSetRemoteGasData {
                    gas_oracle,
                    remote_domain: target.remote.domain_id,
                    token_exchange_rate,
                    gas_price,
                },
            })
        }

        async fn submit_update(
            &self,
            _target: &ReconciliationTarget,
            _plan: &TxPlan,
            _signer: &SignerConfig,
        ) -> Result<TxReceipt> {
            Ok(TxReceipt {
                tx_hash: "0xdeadbeef".to_string(),
                height: Some(7),
            })
        }

        async fn verify_update(
            &self,
            _target: &ReconciliationTarget,
            _expected: &IgpConfig,
        ) -> Result<VerificationResult> {
            Err(IgpOracleError::UnsupportedLiveRead(
                "test verification".to_string(),
            ))
        }

        fn check_signer_authorization(
            &self,
            tx_signer: &TxSigner,
            signer_config: &SignerConfig,
        ) -> Result<SignerAuthStatus> {
            let Some(authorized) = tx_signer.address.as_deref() else {
                return Ok(SignerAuthStatus::AuthorityUnavailable);
            };
            if signer_config.from.to_ascii_lowercase() != authorized.to_ascii_lowercase() {
                return Err(IgpOracleError::InvalidTarget(format!(
                    "configured signer {} is not authorized; expected {authorized}",
                    signer_config.from
                )));
            }
            Ok(SignerAuthStatus::AddressMatch)
        }
    }

    #[tokio::test]
    async fn write_mode_submits_single_cosmosnative_update_with_test_adapter() {
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
            dry_run: false,
            write: true,
            generate_only: false,
        };
        let current = IgpConfig {
            gas_price: "100".to_string(),
            token_exchange_rate: "1".to_string(),
            gas_overhead: 174_289,
        };
        let adapter_factory = move |protocol| {
            Box::new(StaticChainAdapter {
                protocol,
                configs: vec![configured_remote(2_147_483_647, current.clone())],
                fail_plan_update: false,
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
        .expect("write mode should submit through the test adapter");

        assert_eq!(code, 0);
        let plan = std::fs::read_to_string(output_dir.path().join("igp-plan.json")).expect("plan");
        assert!(plan.contains("\"mode\": \"submit\""));
        assert!(plan.contains("\"status\": \"submitted\""));
        assert!(plan.contains("\"txHash\": \"test-tx-hash\""));
        assert!(plan.contains("\"height\": 123"));
    }

    #[tokio::test]
    async fn write_mode_submits_single_evm_update_with_test_adapter() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let output_dir = tempdir().expect("tempdir");
        let mut config =
            UpdaterConfig::load(&repo_root.join("crates/igp-oracle/igp-oracle.example.yaml"))
                .expect("config should load");
        config.targets[0].origin_chain = "edentestnet".to_string();
        config.targets[0].exchange_rate.max = "1000000000000000000000000000".to_string();
        config.targets[0].remote_selection = RemoteSelection::Domains {
            domains: vec![1_297_040_200],
        };
        config.targets[0].write.enabled = true;
        config.targets[0].write.method = WriteMethod::Evm;
        config.targets[0].write.signer_profile = "edentestnet-owner".to_string();
        config.signers.insert(
            "edentestnet-owner".to_string(),
            SignerConfig {
                protocol: ChainProtocol::Ethereum,
                from: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
                key_env: "HYP_KEY_EVM_TEST_UNUSED".to_string(),
            },
        );
        let mut prices = BTreeMap::new();
        prices.insert("edentestnet".to_string(), Decimal::from(2));
        prices.insert("celestiatestnet".to_string(), Decimal::from(4));
        let args = ReconcileArgs {
            config: repo_root.join("crates/igp-oracle/igp-oracle.example.yaml"),
            registry: repo_root,
            origin: Some("edentestnet".to_string()),
            remote_chain: Some("celestiatestnet".to_string()),
            remote_domain: None,
            output_dir: output_dir.path().to_path_buf(),
            format: "markdown,json".to_string(),
            dry_run: false,
            write: true,
            generate_only: false,
        };
        let current = IgpConfig {
            gas_price: "100".to_string(),
            token_exchange_rate: "22000000000000000000000".to_string(),
            gas_overhead: 50_000,
        };
        let adapter_factory = move |protocol| {
            Box::new(NoDiscoveryChainAdapter {
                protocol,
                configs: vec![configured_remote(1_297_040_200, current.clone())],
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
        .expect("EVM write mode should submit through the test adapter");

        let plan = std::fs::read_to_string(output_dir.path().join("igp-plan.json")).expect("plan");
        assert_eq!(code, 0, "{plan}");
        assert!(plan.contains("\"protocol\": \"ethereum\""), "{plan}");
        assert!(plan.contains("\"mode\": \"submit\""), "{plan}");
        assert!(plan.contains("\"status\": \"submitted\""), "{plan}");
        assert!(
            plan.contains("\"transactionModel\": \"single_evm_call\""),
            "{plan}"
        );
        assert!(plan.contains("\"txHash\": \"0xdeadbeef\""), "{plan}");
        assert!(plan.contains("\"height\": 7"), "{plan}");
    }

    #[tokio::test]
    async fn evm_origin_requires_operator_domain_selection() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut config =
            UpdaterConfig::load(&repo_root.join("crates/igp-oracle/igp-oracle.example.yaml"))
                .expect("config should load");
        config.targets[0].origin_chain = "edentestnet".to_string();
        let args = ReconcileArgs {
            config: repo_root.join("crates/igp-oracle/igp-oracle.example.yaml"),
            registry: repo_root,
            origin: Some("edentestnet".to_string()),
            remote_chain: None,
            remote_domain: None,
            output_dir: PathBuf::from("artifacts"),
            format: "markdown,json".to_string(),
            dry_run: true,
            write: false,
            generate_only: false,
        };
        let adapter_factory = |protocol| {
            Box::new(NoDiscoveryChainAdapter {
                protocol,
                configs: Vec::new(),
            }) as Box<dyn ChainAdapter>
        };

        let err = run_reconcile_with_sources_and_adapter_factory(
            args,
            &config,
            &StaticPriceAdapter(BTreeMap::new()),
            &StaticGasAdapter(100),
            &adapter_factory,
        )
        .await
        .expect_err("EVM origin should require explicit domain selection");

        assert!(
            matches!(err, IgpOracleError::InvalidTarget(_)),
            "unexpected error: {err:?}"
        );
        assert!(err.to_string().contains("cannot enumerate"));
        assert!(err.to_string().contains("remoteSelection.domains"));
    }

    #[tokio::test]
    async fn evm_origin_uses_remote_filter_as_operator_domain_selection() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let output_dir = tempdir().expect("tempdir");
        let mut config =
            UpdaterConfig::load(&repo_root.join("crates/igp-oracle/igp-oracle.example.yaml"))
                .expect("config should load");
        config.targets[0].origin_chain = "edentestnet".to_string();
        config.targets[0].exchange_rate.max = "1000000000000000000000000000".to_string();
        let mut prices = BTreeMap::new();
        prices.insert("edentestnet".to_string(), Decimal::from(2));
        prices.insert("celestiatestnet".to_string(), Decimal::from(4));
        let args = ReconcileArgs {
            config: repo_root.join("crates/igp-oracle/igp-oracle.example.yaml"),
            registry: repo_root,
            origin: Some("edentestnet".to_string()),
            remote_chain: Some("celestiatestnet".to_string()),
            remote_domain: None,
            output_dir: output_dir.path().to_path_buf(),
            format: "markdown,json".to_string(),
            dry_run: true,
            write: false,
            generate_only: false,
        };
        let current = IgpConfig {
            gas_price: "110".to_string(),
            token_exchange_rate: "22000000000000000000000".to_string(),
            gas_overhead: 50_000,
        };
        let adapter_factory = move |protocol| {
            Box::new(NoDiscoveryChainAdapter {
                protocol,
                configs: vec![configured_remote(1_297_040_200, current.clone())],
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
        .expect("EVM direct read should reconcile selected remote");

        let plan = std::fs::read_to_string(output_dir.path().join("igp-plan.json")).expect("plan");
        assert_eq!(code, 0, "{plan}");
        assert!(plan.contains("\"originChain\": \"edentestnet\""));
        assert!(plan.contains("\"remoteChain\": \"celestiatestnet\""));
        assert!(plan.contains("\"protocol\": \"ethereum\""));
        assert!(plan.contains("\"query\": \"test-direct-read\""));
        assert!(plan.contains("\"configuredRemoteDomains\": 1"));
    }

    #[tokio::test]
    async fn evm_origin_uses_configured_operator_domain_selection() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let output_dir = tempdir().expect("tempdir");
        let mut config =
            UpdaterConfig::load(&repo_root.join("crates/igp-oracle/igp-oracle.example.yaml"))
                .expect("config should load");
        config.targets[0].origin_chain = "edentestnet".to_string();
        config.targets[0].exchange_rate.max = "1000000000000000000000000000".to_string();
        config.targets[0].remote_selection = RemoteSelection::Domains {
            domains: vec![1_297_040_200],
        };
        let mut prices = BTreeMap::new();
        prices.insert("edentestnet".to_string(), Decimal::from(2));
        prices.insert("celestiatestnet".to_string(), Decimal::from(4));
        let args = ReconcileArgs {
            config: repo_root.join("crates/igp-oracle/igp-oracle.example.yaml"),
            registry: repo_root,
            origin: Some("edentestnet".to_string()),
            remote_chain: None,
            remote_domain: None,
            output_dir: output_dir.path().to_path_buf(),
            format: "markdown,json".to_string(),
            dry_run: true,
            write: false,
            generate_only: false,
        };
        let current = IgpConfig {
            gas_price: "110".to_string(),
            token_exchange_rate: "22000000000000000000000".to_string(),
            gas_overhead: 50_000,
        };
        let adapter_factory = move |protocol| {
            Box::new(NoDiscoveryChainAdapter {
                protocol,
                configs: vec![configured_remote(1_297_040_200, current.clone())],
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
        .expect("EVM configured domain should reconcile");

        let plan = std::fs::read_to_string(output_dir.path().join("igp-plan.json")).expect("plan");
        assert_eq!(code, 0, "{plan}");
        assert!(plan.contains("\"remoteChain\": \"celestiatestnet\""));
        assert!(plan.contains("\"configuredRemoteDomains\": 1"));
        assert!(plan.contains("\"query\": \"test-direct-read\""));
    }

    #[tokio::test]
    async fn dry_run_writes_artifacts() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let output_dir = tempdir().expect("tempdir");
        std::fs::write(output_dir.path().join("tx-plan.json"), "{}\n")
            .expect("stale tx plan fixture");
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
            generate_only: false,
        };

        let current = IgpConfig {
            gas_price: "110".to_string(),
            token_exchange_rate: "1".to_string(),
            gas_overhead: 174_289,
        };
        let adapter_factory = move |protocol| {
            Box::new(StaticChainAdapter {
                protocol,
                configs: vec![configured_remote(2_147_483_647, current.clone())],
                fail_plan_update: false,
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
        assert!(!output_dir.path().join("tx-plan.json").exists());

        let plan = std::fs::read_to_string(output_dir.path().join("igp-plan.json")).expect("plan");
        assert!(plan.contains("\"policy\": {"));
        assert!(plan.contains("\"minBpsChangeToWrite\": 500"));
        assert!(plan.contains("\"maxBpsChangePerUpdate\": 5000"));
        assert!(plan.contains("\"status\": \"noop\""));
        assert!(plan.contains("\"code\": \"noop\""));
        assert!(plan.contains("\"deltaBps\": 0"));
        assert!(!plan.contains("\"minWriteDeltaBps\""));
        assert!(!plan.contains("\"maxAllowedDeltaBps\""));
        assert!(plan.contains("\"gas\": {"));
        assert!(plan.contains("\"prices\": {"));
        assert!(plan.contains("\"onChainRead\": {"));
        assert!(plan.contains("\"current\": {"));
        assert!(plan.contains("\"deltas\": {"));
        assert!(plan.contains("\"gasPriceBps\": 0"));
        assert!(plan.contains("\"sampledGasPrice\": \"100\""));
        assert!(plan.contains("\"proposedGasPrice\": \"110\""));
        assert!(plan.contains("\"source\": \"test\""));
        assert!(plan.contains("\"originNativeTokenDecimals\": 6"));
        assert!(plan.contains("\"remoteNativeTokenDecimals\": 18"));
        assert!(plan.contains("\"tokenDecimalAdjustment\": \"0.000000000001\""));
        assert!(plan.contains("\"gasPrice\": \"110\""));
        assert!(plan.contains("\"tokenExchangeRate\": \"1\""));
        assert!(!plan.contains("\"tx\":"));
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
            generate_only: false,
        };
        let current = IgpConfig {
            gas_price: "100".to_string(),
            token_exchange_rate: "1".to_string(),
            gas_overhead: 174_289,
        };
        let adapter_factory = move |protocol| {
            Box::new(StaticChainAdapter {
                protocol,
                configs: vec![configured_remote(2_147_483_647, current.clone())],
                fail_plan_update: false,
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
        assert!(!plan.contains("\"message\""));
        assert!(!output_dir.path().join("tx-plan.json").exists());
    }

    #[tokio::test]
    async fn generate_only_groups_cosmosnative_updates_as_multi_message_tx() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let output_dir = tempdir().expect("tempdir");
        let mut config =
            UpdaterConfig::load(&repo_root.join("crates/igp-oracle/igp-oracle.example.yaml"))
                .expect("config should load");
        config.targets[0].remote_selection = RemoteSelection::Domains {
            domains: vec![2_147_483_647, 1_000_101],
        };

        let mut prices = BTreeMap::new();
        prices.insert("celestiatestnet".to_string(), Decimal::from(2));
        prices.insert("edentestnet".to_string(), Decimal::from(4));
        prices.insert("xomarkettestnet".to_string(), Decimal::from(3));

        let args = ReconcileArgs {
            config: repo_root.join("crates/igp-oracle/igp-oracle.example.yaml"),
            registry: repo_root,
            origin: Some("celestiatestnet".to_string()),
            remote_chain: None,
            remote_domain: None,
            output_dir: output_dir.path().to_path_buf(),
            format: "markdown,json".to_string(),
            dry_run: false,
            write: true,
            generate_only: true,
        };
        let current = IgpConfig {
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
                fail_plan_update: false,
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
        .expect("cosmosnative generate-only should build a grouped write plan");

        assert_eq!(code, 0);
        let plan = std::fs::read_to_string(output_dir.path().join("igp-plan.json")).expect("plan");
        assert!(plan.contains("\"writePlan\": {"));
        assert!(plan.contains("\"status\": \"ready\""));
        assert!(plan.contains("\"transactionModel\": \"single_cosmos_tx_multi_message\""));
        let write_plan_targets = plan.matches("\"signerProfile\":").count();
        assert!(
            write_plan_targets >= 2,
            "expected at least two grouped targets, got {write_plan_targets}: {plan}"
        );
    }

    #[tokio::test]
    async fn generate_only_rejects_multi_target_evm_updates() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let output_dir = tempdir().expect("tempdir");
        let mut config =
            UpdaterConfig::load(&repo_root.join("crates/igp-oracle/igp-oracle.example.yaml"))
                .expect("config should load");
        config.targets[0].origin_chain = "edentestnet".to_string();
        config.targets[0].remote_selection = RemoteSelection::Domains {
            domains: vec![1_297_040_200, 1_000_101],
        };
        config.targets[0].write.method = WriteMethod::Evm;
        config.defaults.max_bps_change_per_update = 1_000_000_000;
        config.targets[0].exchange_rate.max = "1000000000000000000000000000".to_string();

        let mut prices = BTreeMap::new();
        prices.insert("edentestnet".to_string(), Decimal::from(2));
        prices.insert("celestiatestnet".to_string(), Decimal::from(4));
        prices.insert("xomarkettestnet".to_string(), Decimal::from(3));

        let args = ReconcileArgs {
            config: repo_root.join("crates/igp-oracle/igp-oracle.example.yaml"),
            registry: repo_root,
            origin: Some("edentestnet".to_string()),
            remote_chain: None,
            remote_domain: None,
            output_dir: output_dir.path().to_path_buf(),
            format: "markdown,json".to_string(),
            dry_run: false,
            write: true,
            generate_only: true,
        };
        let current_celestia = IgpConfig {
            gas_price: "100".to_string(),
            token_exchange_rate: "20000000000000000000000".to_string(),
            gas_overhead: 50_000,
        };
        let current_xomarket = IgpConfig {
            gas_price: "100".to_string(),
            token_exchange_rate: "15000000000".to_string(),
            gas_overhead: 50_000,
        };
        let adapter_factory = move |protocol| {
            Box::new(NoDiscoveryChainAdapter {
                protocol,
                configs: vec![
                    configured_remote(1_297_040_200, current_celestia.clone()),
                    configured_remote(1_000_101, current_xomarket.clone()),
                ],
            }) as Box<dyn ChainAdapter>
        };

        let err = run_reconcile_with_sources_and_adapter_factory(
            args,
            &config,
            &StaticPriceAdapter(prices),
            &StaticGasAdapter(100),
            &adapter_factory,
        )
        .await
        .expect_err("EVM generate-only should reject multi-target writes");

        assert!(
            matches!(err, IgpOracleError::InvalidTarget(_)),
            "unexpected error: {err:?}"
        );
        assert!(err
            .to_string()
            .contains("EVM write generate-only supports exactly one"));
    }

    #[tokio::test]
    async fn generate_only_authorizes_single_evm_update() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let output_dir = tempdir().expect("tempdir");
        let mut config =
            UpdaterConfig::load(&repo_root.join("crates/igp-oracle/igp-oracle.example.yaml"))
                .expect("config should load");
        config.targets[0].origin_chain = "edentestnet".to_string();
        config.targets[0].remote_selection = RemoteSelection::Domains {
            domains: vec![1_297_040_200],
        };
        config.targets[0].write.method = WriteMethod::Evm;
        config.targets[0].write.signer_profile = "evm-owner".to_string();
        config.defaults.max_bps_change_per_update = 1_000_000_000;
        config.targets[0].exchange_rate.max = "1000000000000000000000000000".to_string();
        config.signers.insert(
            "evm-owner".to_string(),
            SignerConfig {
                protocol: ChainProtocol::Ethereum,
                from: "0xAaAaAaAaAaAaAaAaAaAaAaAaAaAaAaAaAaAaAaAa".to_string(),
                key_env: "HYP_KEY".to_string(),
            },
        );

        let mut prices = BTreeMap::new();
        prices.insert("edentestnet".to_string(), Decimal::from(2));
        prices.insert("celestiatestnet".to_string(), Decimal::from(4));

        let args = ReconcileArgs {
            config: repo_root.join("crates/igp-oracle/igp-oracle.example.yaml"),
            registry: repo_root,
            origin: Some("edentestnet".to_string()),
            remote_chain: Some("celestiatestnet".to_string()),
            remote_domain: None,
            output_dir: output_dir.path().to_path_buf(),
            format: "markdown,json".to_string(),
            dry_run: false,
            write: true,
            generate_only: true,
        };
        let current = IgpConfig {
            gas_price: "100".to_string(),
            token_exchange_rate: "20000000000000000000000".to_string(),
            gas_overhead: 50_000,
        };
        let adapter_factory = move |protocol| {
            Box::new(NoDiscoveryChainAdapter {
                protocol,
                configs: vec![configured_remote(1_297_040_200, current.clone())],
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
        .expect("EVM generate-only should authorize matching signer");

        assert_eq!(code, 0);
        let plan = std::fs::read_to_string(output_dir.path().join("igp-plan.json")).expect("plan");
        assert!(plan.contains("\"transactionModel\": \"single_evm_call\""));
        assert!(plan.contains("\"status\": \"address_match\""));
        assert!(
            plan.contains("\"configuredSigner\": \"0xAaAaAaAaAaAaAaAaAaAaAaAaAaAaAaAaAaAaAaAa\"")
        );
    }

    #[tokio::test]
    async fn generate_only_rejects_unauthorized_evm_signer() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let output_dir = tempdir().expect("tempdir");
        let mut config =
            UpdaterConfig::load(&repo_root.join("crates/igp-oracle/igp-oracle.example.yaml"))
                .expect("config should load");
        config.targets[0].origin_chain = "edentestnet".to_string();
        config.targets[0].remote_selection = RemoteSelection::Domains {
            domains: vec![1_297_040_200],
        };
        config.targets[0].write.method = WriteMethod::Evm;
        config.targets[0].write.signer_profile = "evm-owner".to_string();
        config.defaults.max_bps_change_per_update = 1_000_000_000;
        config.targets[0].exchange_rate.max = "1000000000000000000000000000".to_string();
        config.signers.insert(
            "evm-owner".to_string(),
            SignerConfig {
                protocol: ChainProtocol::Ethereum,
                from: "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
                key_env: "HYP_KEY".to_string(),
            },
        );

        let mut prices = BTreeMap::new();
        prices.insert("edentestnet".to_string(), Decimal::from(2));
        prices.insert("celestiatestnet".to_string(), Decimal::from(4));

        let args = ReconcileArgs {
            config: repo_root.join("crates/igp-oracle/igp-oracle.example.yaml"),
            registry: repo_root,
            origin: Some("edentestnet".to_string()),
            remote_chain: Some("celestiatestnet".to_string()),
            remote_domain: None,
            output_dir: output_dir.path().to_path_buf(),
            format: "markdown,json".to_string(),
            dry_run: false,
            write: true,
            generate_only: true,
        };
        let current = IgpConfig {
            gas_price: "100".to_string(),
            token_exchange_rate: "20000000000000000000000".to_string(),
            gas_overhead: 50_000,
        };
        let adapter_factory = move |protocol| {
            Box::new(NoDiscoveryChainAdapter {
                protocol,
                configs: vec![configured_remote(1_297_040_200, current.clone())],
            }) as Box<dyn ChainAdapter>
        };

        let err = run_reconcile_with_sources_and_adapter_factory(
            args,
            &config,
            &StaticPriceAdapter(prices),
            &StaticGasAdapter(100),
            &adapter_factory,
        )
        .await
        .expect_err("EVM generate-only should reject a non-owner signer");

        assert!(
            matches!(err, IgpOracleError::InvalidTarget(_)),
            "unexpected error: {err:?}"
        );
        assert!(err.to_string().contains("is not authorized"));
    }

    #[tokio::test]
    async fn dry_run_keeps_proposal_when_tx_planning_fails() {
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
            generate_only: false,
        };
        let current = IgpConfig {
            gas_price: "100".to_string(),
            token_exchange_rate: "1".to_string(),
            gas_overhead: 174_289,
        };
        let adapter_factory = move |protocol| {
            Box::new(StaticChainAdapter {
                protocol,
                configs: vec![configured_remote(2_147_483_647, current.clone())],
                fail_plan_update: true,
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
        .expect("dry-run should preserve evaluated proposal");

        assert_eq!(code, 10);
        let plan = std::fs::read_to_string(output_dir.path().join("igp-plan.json")).expect("plan");
        assert!(plan.contains("\"status\": \"update_recommended\""));
        assert!(plan.contains("\"proposed\": {"));
        assert!(!plan.contains("\"tx\":"));
        assert!(plan.contains("\"txPlanError\": {"));
        assert!(plan.contains("test tx plan source failed"));
    }

    #[tokio::test]
    async fn dry_run_records_target_errors_by_category_and_continues() {
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
            generate_only: false,
        };
        let current = IgpConfig {
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
                fail_plan_update: false,
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
        assert!(plan.contains("\"status\": \"market_data_error\""));
        assert!(!plan.contains("\"code\": \"market_data_error\""));
        assert!(plan.contains("missing static price for xomarkettestnet"));
        assert!(plan.contains("\"status\": \"update_recommended\""));
    }

    #[test]
    fn classifies_target_errors_for_artifacts() {
        assert_eq!(
            IgpOracleError::InvalidConfig("missing mapping".to_string()).classify(),
            DecisionStatus::ConfigError
        );
        assert_eq!(
            IgpOracleError::MarketData("CoinGecko returned an error: 429".to_string()).classify(),
            DecisionStatus::MarketDataError
        );
        assert_eq!(
            IgpOracleError::GasData("eth_gasPrice HTTP error".to_string()).classify(),
            DecisionStatus::GasDataError
        );
        assert_eq!(
            IgpOracleError::OnchainRead("destination gas config gRPC query failed".to_string())
                .classify(),
            DecisionStatus::OnchainReadError
        );
        assert_eq!(
            IgpOracleError::Policy("invalid current gasPrice".to_string()).classify(),
            DecisionStatus::PolicyError
        );
        assert_eq!(
            IgpOracleError::DataSource("uncategorized".to_string()).classify(),
            DecisionStatus::DataSourceError
        );
    }

    fn configured_remote(remote_domain: u32, current: IgpConfig) -> ConfiguredRemoteDomain {
        ConfiguredRemoteDomain {
            remote_domain,
            current,
            source: OnChainReadSource {
                protocol: ChainProtocol::CosmosNative,
                endpoint: Some("test://endpoint".to_string()),
                query: "test-query".to_string(),
            },
        }
    }
}
