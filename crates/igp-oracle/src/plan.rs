use crate::{
    adapter::{ChainAdapter, ChainAdapterFactory},
    artifacts::{
        PlanArtifact, SignerAuthorizationArtifact, TargetPlanArtifact, TransactionModel,
        TxPlanErrorArtifact, WritePlanArtifact, WritePlanMode, WritePlanStatus,
        WritePlanTargetArtifact, WriteReceiptArtifact,
    },
    config::UpdaterConfig,
    error::{IgpOracleError, Result},
    models::{ChainProtocol, ReconciliationTarget, TxPlan},
    policy::DecisionStatus,
};

pub(crate) struct ExecutableUpdate {
    pub target: ReconciliationTarget,
    pub tx: TxPlan,
}

pub(crate) fn build_write_plan(
    config: &UpdaterConfig,
    plan: &PlanArtifact,
    mode: WritePlanMode,
    adapter_factory: &ChainAdapterFactory,
) -> Result<WritePlanArtifact> {
    if plan
        .targets
        .iter()
        .any(|target| target.decision.status.is_error())
    {
        return Err(IgpOracleError::InvalidTarget(
            "write generate-only requires all selected targets to evaluate without errors"
                .to_string(),
        ));
    }

    if plan
        .targets
        .iter()
        .any(|target| target.decision.status == DecisionStatus::PolicyViolation)
    {
        return Err(IgpOracleError::Policy(
            "write generate-only is blocked by a policy violation".to_string(),
        ));
    }

    let update_targets = plan
        .targets
        .iter()
        .filter(|target| target.decision.status == DecisionStatus::UpdateRecommended)
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
            .map(|origin| origin.protocol)
            .unwrap_or(ChainProtocol::Ethereum);
        return Ok(WritePlanArtifact {
            mode,
            status: WritePlanStatus::NoUpdateRequired,
            protocol,
            origin_chain,
            transaction_model: TransactionModel::None,
            targets: Vec::new(),
            receipts: Vec::new(),
            error: None,
        });
    }

    let origin_chain = update_targets[0].origin_chain.clone();
    let protocol = update_targets[0].origin_protocol;
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

    let transaction_model = match protocol {
        ChainProtocol::CosmosNative => TransactionModel::SingleCosmosTxMultiMessage,
        ChainProtocol::Ethereum => {
            if update_targets.len() != 1 {
                return Err(IgpOracleError::InvalidTarget(
                    "EVM write generate-only supports exactly one update target; use --remote-chain or --remote-domain to select a single remote"
                        .to_string(),
                ));
            }
            TransactionModel::SingleEvmCall
        }
    };

    let targets = update_targets
        .iter()
        .map(|target| {
            let tx = target
                .tx
                .as_ref()
                .expect("transaction plan presence is validated above");
            let signer_authorization = build_signer_authorization(
                config,
                target,
                tx,
                adapter_factory(target.origin_protocol).as_ref(),
            )?;
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
        mode,
        status: WritePlanStatus::Ready,
        protocol,
        origin_chain,
        transaction_model,
        targets,
        receipts: Vec::new(),
        error: None,
    })
}

pub(crate) fn mark_write_plan_failed(plan: &mut PlanArtifact, err: &IgpOracleError) {
    let Some(write_plan) = plan.write_plan.as_mut() else {
        return;
    };
    write_plan.status = WritePlanStatus::Failed;
    write_plan.error = Some(TxPlanErrorArtifact {
        status: err.classify(),
        reason: err.to_string(),
    });
}

pub(crate) async fn submit_write_plan(
    config: &UpdaterConfig,
    plan: &mut PlanArtifact,
    executable_updates: Vec<ExecutableUpdate>,
    adapter_factory: &ChainAdapterFactory,
) -> Result<()> {
    let write_plan = plan.write_plan.as_mut().ok_or_else(|| {
        IgpOracleError::InvalidTarget("write mode requires a generated write plan".to_string())
    })?;

    if write_plan.status == WritePlanStatus::NoUpdateRequired {
        return Ok(());
    }

    if executable_updates.len() != write_plan.targets.len() {
        return Err(IgpOracleError::InvalidTarget(format!(
            "write plan has {} target(s), but {} executable update(s) were prepared",
            write_plan.targets.len(),
            executable_updates.len()
        )));
    }

    if executable_updates.len() != 1 {
        return Err(IgpOracleError::InvalidTarget(format!(
            "{} submit mode currently supports exactly one update target; use --remote-chain or --remote-domain, or run --generate-only for multi-message review",
            write_plan.protocol.as_str()
        )));
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

    write_plan.status = WritePlanStatus::Submitted;
    write_plan.receipts.push(WriteReceiptArtifact {
        remote_chain: update.target.remote.name,
        remote_domain: update.target.remote.domain_id,
        tx_hash: receipt.tx_hash,
        height: receipt.height,
    });

    Ok(())
}

fn build_signer_authorization(
    config: &UpdaterConfig,
    target: &TargetPlanArtifact,
    tx: &TxPlan,
    adapter: &dyn ChainAdapter,
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
    let signer_protocol = signer_config.protocol;
    if signer_protocol != tx.protocol || signer_protocol != target.origin_protocol {
        return Err(IgpOracleError::InvalidConfig(format!(
            "signer profile {} uses protocol {}, but target {} -> {} uses {}",
            tx_signer.signer_profile,
            signer_protocol.as_str(),
            target.origin_chain,
            target.remote_chain,
            target.origin_protocol.as_str()
        )));
    }

    let status = adapter.check_signer_authorization(tx_signer, signer_config)?;

    Ok(SignerAuthorizationArtifact {
        signer_profile: tx_signer.signer_profile.clone(),
        configured_signer: signer_config.from.clone(),
        authorized_signer: tx_signer.address.clone(),
        status,
    })
}
