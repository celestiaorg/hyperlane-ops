use std::{collections::BTreeSet, path::Path, process::Command};

use crate::{
    adapter::{adapter_for, ChainAdapter, GasAdapter, PriceAdapter},
    artifacts::{
        target_artifact, write_artifacts, DecisionArtifact, DiscoveryArtifact, PlanArtifact,
        PolicyArtifact, SignerAuthorizationArtifact, SkippedTargetArtifact, TargetArtifactInput,
        TxPlanErrorArtifact, WritePlanArtifact, WritePlanTargetArtifact, WriteReceiptArtifact,
    },
    cli::ReconcileArgs,
    config::UpdaterConfig,
    data::{CoinGeckoPriceAdapter, ProtocolGasAdapter},
    error::{IgpOracleError, Result},
    models::{ChainProtocol, ReconciliationTarget, TxPlan},
    policy::{compute_proposal, decide_reconciliation},
    registry::RegistryIndex,
    resolver::{
        expand_configured_domains, resolve_origin_work_items, selected_remote_domain_filter,
        ExpandedTarget, OriginWorkItem,
    },
};

type ChainAdapterFactory = dyn Fn(ChainProtocol) -> Box<dyn ChainAdapter> + Sync;

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
    protocol: String,
    configured_count: usize,
    adapter: Box<dyn ChainAdapter>,
    expanded: Vec<ExpandedTarget>,
}

struct ExecutableUpdate {
    target: ReconciliationTarget,
    tx: TxPlan,
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
            protocol: work_item.origin.protocol.as_str().to_string(),
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
                let (status, code) = target_error_classification(&err);
                expanded.push(ExpandedTarget::ReadError {
                    target: Box::new(target),
                    status: status.to_string(),
                    code: code.to_string(),
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
                    if reconciled.artifact.decision.status == "update_recommended" {
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
                        status: "skipped".to_string(),
                        code,
                        reason,
                    });
                }
                ExpandedTarget::ReadError {
                    target,
                    status,
                    code,
                    reason,
                } => {
                    resolved_count += 1;
                    target_artifacts.push(target_read_error_artifact(
                        &target, config, status, code, reason,
                    ));
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
            "generate_only"
        } else {
            "submit"
        };
        plan.write_plan = Some(build_write_plan(config, &plan, mode)?);
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

fn build_write_plan(
    config: &UpdaterConfig,
    plan: &PlanArtifact,
    mode: &str,
) -> Result<WritePlanArtifact> {
    if plan
        .targets
        .iter()
        .any(|target| target.decision.status.ends_with("_error"))
    {
        return Err(IgpOracleError::InvalidTarget(
            "write generate-only requires all selected targets to evaluate without errors"
                .to_string(),
        ));
    }

    if plan
        .targets
        .iter()
        .any(|target| target.decision.status == "policy_violation")
    {
        return Err(IgpOracleError::Policy(
            "write generate-only is blocked by a policy violation".to_string(),
        ));
    }

    let update_targets = plan
        .targets
        .iter()
        .filter(|target| target.decision.status == "update_recommended")
        .collect::<Vec<_>>();
    if update_targets.is_empty() {
        let origin_chain = plan
            .discovery
            .first()
            .map(|origin| origin.origin_chain.clone())
            .unwrap_or_else(|| "unknown".to_string());
        let protocol = plan
            .discovery
            .first()
            .map(|origin| origin.protocol.clone())
            .unwrap_or_else(|| "unknown".to_string());
        return Ok(WritePlanArtifact {
            mode: mode.to_string(),
            status: "no_update_required".to_string(),
            protocol,
            origin_chain,
            transaction_model: "none".to_string(),
            target_count: 0,
            message_count: 0,
            targets: Vec::new(),
            receipts: Vec::new(),
            error: None,
        });
    }

    let origin_chain = update_targets[0].origin_chain.clone();
    let protocol = update_targets[0].origin_protocol.clone();
    if update_targets
        .iter()
        .any(|target| target.origin_chain != origin_chain || target.origin_protocol != protocol)
    {
        return Err(IgpOracleError::InvalidTarget(
            "write generate-only requires selected updates to share one origin chain and protocol"
                .to_string(),
        ));
    }

    if let Some(target) = update_targets.iter().find(|target| !target.write_enabled) {
        return Err(IgpOracleError::InvalidTarget(format!(
            "write generate-only is disabled in config for origin {} remote {}",
            target.origin_chain, target.remote_chain
        )));
    }

    if let Some(target) = update_targets.iter().find(|target| target.tx.is_none()) {
        return Err(IgpOracleError::InvalidTarget(format!(
            "write generate-only requires a transaction plan for origin {} remote {}",
            target.origin_chain, target.remote_chain
        )));
    }

    let transaction_model = match protocol.as_str() {
        "cosmosnative" => "single_cosmos_tx_multi_message",
        "ethereum" => {
            if update_targets.len() != 1 {
                return Err(IgpOracleError::InvalidTarget(
                    "EVM write generate-only supports exactly one update target; use --remote-chain or --remote-domain to select a single remote"
                        .to_string(),
                ));
            }
            "single_evm_call"
        }
        _ => {
            return Err(IgpOracleError::UnsupportedProtocol(format!(
                "write generate-only is unsupported for protocol {protocol}"
            )))
        }
    };

    let targets = update_targets
        .iter()
        .map(|target| {
            let tx = target
                .tx
                .as_ref()
                .expect("transaction plan presence is validated above");
            let signer_authorization = signer_authorization(config, target, tx)?;
            Ok(WritePlanTargetArtifact {
                remote_chain: target.remote_chain.clone(),
                remote_domain: target.remote_domain,
                action: tx.action.clone(),
                target: tx.target.clone(),
                selector: tx.selector.clone(),
                signer_authorization,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(WritePlanArtifact {
        mode: mode.to_string(),
        status: "ready".to_string(),
        protocol,
        origin_chain,
        transaction_model: transaction_model.to_string(),
        target_count: targets.len(),
        message_count: targets.len(),
        targets,
        receipts: Vec::new(),
        error: None,
    })
}

fn mark_write_plan_failed(plan: &mut PlanArtifact, err: &IgpOracleError) {
    let Some(write_plan) = plan.write_plan.as_mut() else {
        return;
    };
    let (_, code) = target_error_classification(err);
    write_plan.status = "failed".to_string();
    write_plan.error = Some(TxPlanErrorArtifact {
        code: code.to_string(),
        reason: err.to_string(),
    });
}

async fn submit_write_plan(
    config: &UpdaterConfig,
    plan: &mut PlanArtifact,
    executable_updates: Vec<ExecutableUpdate>,
    adapter_factory: &ChainAdapterFactory,
) -> Result<()> {
    let write_plan = plan.write_plan.as_mut().ok_or_else(|| {
        IgpOracleError::InvalidTarget("write mode requires a generated write plan".to_string())
    })?;

    if write_plan.status == "no_update_required" {
        write_plan.status = "no_update_required".to_string();
        return Ok(());
    }

    if write_plan.protocol != "cosmosnative" {
        return Err(IgpOracleError::UnsupportedWrite);
    }

    if executable_updates.len() != write_plan.target_count {
        return Err(IgpOracleError::InvalidTarget(format!(
            "write plan has {} target(s), but {} executable update(s) were prepared",
            write_plan.target_count,
            executable_updates.len()
        )));
    }

    if executable_updates.len() != 1 {
        return Err(IgpOracleError::InvalidTarget(
            "cosmosnative submit mode currently supports exactly one update target; use --remote-chain or --remote-domain, or run --generate-only for multi-message review"
                .to_string(),
        ));
    }

    let update = executable_updates
        .into_iter()
        .next()
        .expect("exactly one executable update is validated above");
    let tx_signer = update.tx.signer.as_ref().ok_or_else(|| {
        IgpOracleError::InvalidTarget(format!(
            "write target {} -> {} has no signer plan",
            update.target.origin.name, update.target.remote.name
        ))
    })?;
    let signer_config = config
        .signers
        .get(&tx_signer.signer_profile)
        .ok_or_else(|| {
            IgpOracleError::InvalidConfig(format!(
                "signer profile {} is not configured",
                tx_signer.signer_profile
            ))
        })?;
    let adapter = adapter_factory(update.target.origin.protocol);
    let receipt = adapter
        .submit_update(&update.target, &update.tx, signer_config)
        .await?;

    write_plan.status = "submitted".to_string();
    write_plan.receipts.push(WriteReceiptArtifact {
        remote_chain: update.target.remote.name,
        remote_domain: update.target.remote.domain_id,
        tx_hash: receipt.tx_hash,
        height: receipt.height,
    });

    Ok(())
}

fn signer_authorization(
    config: &UpdaterConfig,
    target: &crate::artifacts::TargetPlanArtifact,
    tx: &crate::models::TxPlan,
) -> Result<SignerAuthorizationArtifact> {
    let tx_signer = tx.signer.as_ref().ok_or_else(|| {
        IgpOracleError::InvalidTarget(format!(
            "write generate-only target {} -> {} has no signer plan",
            target.origin_chain, target.remote_chain
        ))
    })?;
    let signer_config = config
        .signers
        .get(&tx_signer.signer_profile)
        .ok_or_else(|| {
            IgpOracleError::InvalidConfig(format!(
                "signer profile {} is not configured",
                tx_signer.signer_profile
            ))
        })?;
    let signer_protocol = signer_config.protocol.as_str();
    if signer_protocol != tx.protocol || signer_protocol != target.origin_protocol {
        return Err(IgpOracleError::InvalidConfig(format!(
            "signer profile {} uses protocol {}, but target {} -> {} uses {}",
            tx_signer.signer_profile,
            signer_protocol,
            target.origin_chain,
            target.remote_chain,
            target.origin_protocol
        )));
    }

    let status = match target.origin_protocol.as_str() {
        "ethereum" => {
            let authorized = tx_signer.address.as_deref().ok_or_else(|| {
                IgpOracleError::InvalidTarget(format!(
                    "EVM write generate-only target {} -> {} has no authorized signer address",
                    target.origin_chain, target.remote_chain
                ))
            })?;
            if normalize_evm_address_for_compare(&signer_config.from, "configured signer")?
                != normalize_evm_address_for_compare(authorized, "authorized signer")?
            {
                return Err(IgpOracleError::InvalidTarget(format!(
                    "configured signer {} is not authorized for EVM target {} -> {}; expected {}",
                    signer_config.from, target.origin_chain, target.remote_chain, authorized
                )));
            }
            "address_match"
        }
        "cosmosnative" => {
            if let Some(authorized) = tx_signer.address.as_deref() {
                if is_probable_cosmos_address(&signer_config.from) {
                    if signer_config.from != authorized {
                        return Err(IgpOracleError::InvalidTarget(format!(
                            "configured signer {} is not authorized for cosmosnative target {} -> {}; expected {}",
                            signer_config.from, target.origin_chain, target.remote_chain, authorized
                        )));
                    }
                    "address_match"
                } else {
                    "key_alias_unverified"
                }
            } else {
                "authority_unavailable"
            }
        }
        protocol => {
            return Err(IgpOracleError::UnsupportedProtocol(format!(
                "signer authorization is unsupported for protocol {protocol}"
            )));
        }
    };

    Ok(SignerAuthorizationArtifact {
        signer_profile: tx_signer.signer_profile.clone(),
        configured_signer: signer_config.from.clone(),
        authorized_signer: tx_signer.address.clone(),
        status: status.to_string(),
    })
}

fn normalize_evm_address_for_compare(value: &str, label: &str) -> Result<String> {
    let raw = value.strip_prefix("0x").unwrap_or(value);
    if raw.len() != 40 || !raw.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(IgpOracleError::InvalidConfig(format!(
            "{label} must be a 20-byte EVM address, got {value}"
        )));
    }

    Ok(raw.to_ascii_lowercase())
}

fn is_probable_cosmos_address(value: &str) -> bool {
    value.len() > 20 && value.contains('1')
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
        status: reconciliation.status.as_str().to_string(),
        code: reconciliation.code.as_str().to_string(),
        field: reconciliation.field.map(|field| field.as_str().to_string()),
        delta_bps: reconciliation.observed_delta_bps,
        min_write_delta_bps: Some(config.defaults.min_bps_change_to_write),
        max_allowed_delta_bps: Some(config.defaults.max_bps_change_per_update),
        reason: reconciliation.reason,
    };
    let (tx, tx_plan_error) = if decision.status != "noop" {
        match adapter.plan_update(&target, &proposal.proposed).await {
            Ok(plan) => (Some(plan), None),
            Err(err) => {
                let (_, code) = target_error_classification(&err);
                (
                    None,
                    Some(TxPlanErrorArtifact {
                        code: code.to_string(),
                        reason: err.to_string(),
                    }),
                )
            }
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
    let (status, code) = target_error_classification(err);
    target_artifact(TargetArtifactInput {
        target,
        config,
        proposal: None,
        current_read: Some(current_read),
        deltas: None,
        tx: None,
        tx_plan_error: None,
        decision: DecisionArtifact {
            status: status.to_string(),
            code: code.to_string(),
            field: None,
            delta_bps: None,
            min_write_delta_bps: None,
            max_allowed_delta_bps: None,
            reason: err.to_string(),
        },
    })
}

fn target_read_error_artifact(
    target: &crate::models::ReconciliationTarget,
    config: &UpdaterConfig,
    status: String,
    code: String,
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
            code,
            field: None,
            delta_bps: None,
            min_write_delta_bps: None,
            max_allowed_delta_bps: None,
            reason,
        },
    })
}

fn target_error_classification(err: &IgpOracleError) -> (&'static str, &'static str) {
    match err {
        IgpOracleError::InvalidConfig(_)
        | IgpOracleError::Registry(_)
        | IgpOracleError::InvalidTarget(_)
        | IgpOracleError::UnsupportedProtocol(_) => ("config_error", "config_error"),
        IgpOracleError::Policy(_) => ("policy_error", "policy_error"),
        IgpOracleError::UnsupportedLiveRead(_) => ("onchain_read_error", "onchain_read_error"),
        IgpOracleError::DataSource(message) => classify_data_source_error(message),
        IgpOracleError::Io { .. } | IgpOracleError::Yaml { .. } | IgpOracleError::Json { .. } => {
            ("artifact_error", "artifact_error")
        }
        IgpOracleError::UnsupportedWrite => ("write_error", "write_error"),
    }
}

fn classify_data_source_error(message: &str) -> (&'static str, &'static str) {
    let normalized = message.to_ascii_lowercase();
    if normalized.contains("coingecko")
        || normalized.contains("price for asset")
        || normalized.contains("missing static price")
    {
        return ("market_data_error", "market_data_error");
    }

    if normalized.contains("eth_gasprice")
        || normalized.contains("gasprice")
        || normalized.contains("gas price")
        || normalized.contains("remote gas")
    {
        return ("gas_data_error", "gas_data_error");
    }

    if normalized.contains("grpc")
        || normalized.contains("igp")
        || normalized.contains("destination gas config")
    {
        return ("onchain_read_error", "onchain_read_error");
    }

    ("data_source_error", "data_source_error")
}

fn exit_code_for_plan(plan: &PlanArtifact) -> i32 {
    if plan.targets.iter().any(|target| {
        matches!(
            target.decision.status.as_str(),
            "policy_violation" | "policy_error"
        )
    }) {
        return 30;
    }

    if plan
        .targets
        .iter()
        .any(|target| target.decision.status.ends_with("_error"))
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
        adapter::{ChainAdapter, GasAdapter, PriceAdapter},
        cli::ReconcileArgs,
        config::{RemoteSelection, SignerConfig},
        models::{
            ChainMetadata, ConfiguredRemoteDomain, CoreAddresses, CurrentIgpConfig, GasPriceSample,
            IgpConfigRead, OnChainReadSource, ProposedIgpConfig, ReconciliationTarget, TxPlan,
            TxReceipt, TxSigner, VerificationResult,
        },
    };

    use super::*;

    struct StaticGasAdapter(u128);

    #[async_trait]
    impl GasAdapter for StaticGasAdapter {
        async fn remote_gas_price(&self, target: &ReconciliationTarget) -> Result<GasPriceSample> {
            Ok(GasPriceSample {
                source: "test".to_string(),
                remote_chain: target.remote.name.clone(),
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
                IgpOracleError::DataSource(format!("missing static price for {chain_name}"))
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
            if self.fail_plan_update {
                return Err(IgpOracleError::DataSource(
                    "test tx plan source failed".to_string(),
                ));
            }

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
            _expected: &ProposedIgpConfig,
        ) -> Result<VerificationResult> {
            Err(IgpOracleError::UnsupportedLiveRead(
                "test verification".to_string(),
            ))
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
                    IgpOracleError::DataSource(format!(
                        "test adapter has no gas oracle configured for remote domain {}",
                        target.remote.domain_id
                    ))
                })?;

            Ok(IgpConfigRead {
                config,
                source: OnChainReadSource {
                    protocol: self.protocol.as_str().to_string(),
                    endpoint: Some("test://evm-rpc".to_string()),
                    query: "test-direct-read".to_string(),
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
                action: "setRemoteGasData".to_string(),
                message_type: "setRemoteGasData((uint32,uint128,uint128))".to_string(),
                target: "0x1111111111111111111111111111111111111111".to_string(),
                selector: Some("0xf3a1495f".to_string()),
                calldata: Some(format!("0xtest{:x}", target.remote.domain_id)),
                command: None,
                signer: Some(TxSigner {
                    signer_profile: target.config.write.signer_profile.clone(),
                    address: Some("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()),
                }),
                message: serde_json::json!({
                    "remoteGasData": {
                        "remoteDomain": target.remote.domain_id,
                        "tokenExchangeRate": proposed.token_exchange_rate.as_str(),
                        "gasPrice": proposed.gas_price.as_str()
                    }
                }),
                notes: vec!["test evm tx plan".to_string()],
            })
        }

        async fn submit_update(
            &self,
            _target: &ReconciliationTarget,
            _plan: &TxPlan,
            _signer: &SignerConfig,
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
        let current = CurrentIgpConfig {
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
        let current = CurrentIgpConfig {
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
        let current = CurrentIgpConfig {
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

        let current = CurrentIgpConfig {
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
        assert!(plan.contains("\"status\": \"noop\""));
        assert!(plan.contains("\"code\": \"noop\""));
        assert!(plan.contains("\"deltaBps\": 0"));
        assert!(plan.contains("\"minWriteDeltaBps\": 500"));
        assert!(plan.contains("\"maxAllowedDeltaBps\": 5000"));
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
            generate_only: false,
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
        assert!(plan.contains("\"destinationGasConfig\""));
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
        assert!(plan.contains("\"targetCount\": 2"));
        assert!(plan.contains("\"messageCount\": 2"));
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
        config.targets[0].write.method = "evm".to_string();
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
        let current_celestia = CurrentIgpConfig {
            gas_price: "100".to_string(),
            token_exchange_rate: "20000000000000000000000".to_string(),
            gas_overhead: 50_000,
        };
        let current_xomarket = CurrentIgpConfig {
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
        config.targets[0].write.method = "evm".to_string();
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
        let current = CurrentIgpConfig {
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
        config.targets[0].write.method = "evm".to_string();
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
        let current = CurrentIgpConfig {
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
        let current = CurrentIgpConfig {
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
        assert!(plan.contains("\"tx\": null"));
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
        assert!(plan.contains("\"code\": \"market_data_error\""));
        assert!(plan.contains("missing static price for xomarkettestnet"));
        assert!(plan.contains("\"status\": \"update_recommended\""));
    }

    #[test]
    fn classifies_target_errors_for_artifacts() {
        assert_eq!(
            target_error_classification(&IgpOracleError::InvalidConfig(
                "missing mapping".to_string()
            )),
            ("config_error", "config_error")
        );
        assert_eq!(
            target_error_classification(&IgpOracleError::DataSource(
                "CoinGecko returned an error: 429".to_string()
            )),
            ("market_data_error", "market_data_error")
        );
        assert_eq!(
            target_error_classification(&IgpOracleError::DataSource(
                "eth_gasPrice HTTP error".to_string()
            )),
            ("gas_data_error", "gas_data_error")
        );
        assert_eq!(
            target_error_classification(&IgpOracleError::DataSource(
                "destination gas config gRPC query failed".to_string()
            )),
            ("onchain_read_error", "onchain_read_error")
        );
        assert_eq!(
            target_error_classification(&IgpOracleError::Policy(
                "invalid current gasPrice".to_string()
            )),
            ("policy_error", "policy_error")
        );
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
