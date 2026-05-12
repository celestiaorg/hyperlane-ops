use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::{
    adapter::SignerAuthStatus,
    config::{DefaultsConfig, UpdaterConfig, WriteMethod},
    error::{create_dir_all, write, IgpOracleError, Result},
    models::{
        ChainProtocol, IgpConfig, IgpConfigRead, OnChainReadSource, ProposalComputation,
        ReconciliationDelta, ReconciliationTarget, TxPlan,
    },
    policy::{DecisionCode, DecisionStatus, ReconciliationField},
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanArtifact {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_sha: Option<String>,
    pub policy: PolicyArtifact,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub write_plan: Option<WritePlanArtifact>,
    #[serde(default)]
    pub discovery: Vec<DiscoveryArtifact>,
    #[serde(default)]
    pub skipped_targets: Vec<SkippedTargetArtifact>,
    pub targets: Vec<TargetPlanArtifact>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetPlanArtifact {
    pub origin_chain: String,
    pub remote_chain: String,
    pub remote_domain: u32,
    pub origin_protocol: ChainProtocol,
    pub remote_protocol: ChainProtocol,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub igp_identifier: Option<String>,
    pub write_enabled: bool,
    pub write_method: WriteMethod,
    pub gas_overhead: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas: Option<GasInputsArtifact>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prices: Option<PriceInputsArtifact>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_chain_read: Option<OnChainReadSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current: Option<IgpConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposed: Option<IgpConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deltas: Option<ReconciliationDelta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx: Option<TxPlan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_plan_error: Option<TxPlanErrorArtifact>,
    pub decision: DecisionArtifact,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyArtifact {
    pub min_bps_change_to_write: u64,
    pub max_bps_change_per_update: u64,
    pub cooldown_seconds: u64,
    pub safety_multiplier_bps: u64,
    pub gas_sample_freshness_seconds: u64,
}

impl From<&DefaultsConfig> for PolicyArtifact {
    fn from(defaults: &DefaultsConfig) -> Self {
        Self {
            min_bps_change_to_write: defaults.min_bps_change_to_write,
            max_bps_change_per_update: defaults.max_bps_change_per_update,
            cooldown_seconds: defaults.cooldown_seconds,
            safety_multiplier_bps: defaults.safety_multiplier_bps,
            gas_sample_freshness_seconds: defaults.gas_sample_freshness_seconds,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionArtifact {
    pub status: DecisionStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<DecisionCode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<ReconciliationField>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta_bps: Option<u128>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TxPlanErrorArtifact {
    pub status: DecisionStatus,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WritePlanMode {
    Submit,
    GenerateOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WritePlanStatus {
    Ready,
    Submitted,
    Failed,
    NoUpdateRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionModel {
    SingleEvmCall,
    SingleCosmosTxMultiMessage,
    None,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WritePlanArtifact {
    pub mode: WritePlanMode,
    pub status: WritePlanStatus,
    pub protocol: ChainProtocol,
    pub origin_chain: String,
    pub transaction_model: TransactionModel,
    pub targets: Vec<WritePlanTargetArtifact>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub receipts: Vec<WriteReceiptArtifact>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<TxPlanErrorArtifact>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WritePlanTargetArtifact {
    pub remote_chain: String,
    pub remote_domain: u32,
    pub action: String,
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
    pub signer_authorization: SignerAuthorizationArtifact,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteReceiptArtifact {
    pub remote_chain: String,
    pub remote_domain: u32,
    pub tx_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignerAuthorizationArtifact {
    pub signer_profile: String,
    pub configured_signer: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorized_signer: Option<String>,
    pub status: SignerAuthStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryArtifact {
    pub origin_chain: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub igp_identifier: Option<String>,
    pub protocol: ChainProtocol,
    pub configured_remote_domains: usize,
    pub resolved_remote_domains: usize,
    pub skipped_remote_domains: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedTargetArtifact {
    pub origin_chain: String,
    pub remote_domain: u32,
    pub code: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GasInputsArtifact {
    pub mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_amount: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_denom: Option<String>,
    pub sampled_gas_price: String,
    pub proposed_gas_price: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rounding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceInputsArtifact {
    pub price_provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin_market_asset: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_market_asset: Option<String>,
    pub origin_price_usd: String,
    pub remote_price_usd: String,
    pub origin_native_token_decimals: u8,
    pub remote_native_token_decimals: u8,
    pub token_decimal_adjustment: String,
}

pub struct TargetArtifactInput<'a> {
    pub target: &'a ReconciliationTarget,
    pub config: &'a UpdaterConfig,
    pub proposal: Option<ProposalComputation>,
    pub current_read: Option<IgpConfigRead>,
    pub deltas: Option<ReconciliationDelta>,
    pub tx: Option<TxPlan>,
    pub tx_plan_error: Option<TxPlanErrorArtifact>,
    pub decision: DecisionArtifact,
}

pub fn target_artifact(input: TargetArtifactInput<'_>) -> TargetPlanArtifact {
    let gas = input.proposal.as_ref().map(gas_inputs);
    let prices = input
        .proposal
        .as_ref()
        .map(|proposal| price_inputs(input.target, input.config, proposal));
    let on_chain_read = input.current_read.as_ref().map(|read| read.source.clone());
    let current = input.current_read.map(|read| read.config);
    let proposed = input.proposal.map(|proposal| proposal.proposed);

    TargetPlanArtifact {
        origin_chain: input.target.origin.name.clone(),
        remote_chain: input.target.remote.name.clone(),
        remote_domain: input.target.remote.domain_id,
        origin_protocol: input.target.origin.protocol,
        remote_protocol: input.target.remote.protocol,
        igp_identifier: input
            .target
            .origin_addresses
            .interchain_gas_paymaster
            .clone(),
        write_enabled: input.target.config.write.enabled,
        write_method: input.target.config.write.method,
        gas_overhead: input.target.gas_overhead,
        gas,
        prices,
        on_chain_read,
        current,
        proposed,
        deltas: input.deltas,
        tx: input.tx,
        tx_plan_error: input.tx_plan_error,
        decision: input.decision,
    }
}

fn gas_inputs(proposal: &ProposalComputation) -> GasInputsArtifact {
    match proposal.gas.as_ref() {
        Some(sample) => GasInputsArtifact {
            mode: proposal.gas_mode.clone(),
            source: Some(sample.source.clone()),
            raw_amount: sample.raw_amount.clone(),
            raw_denom: sample.raw_denom.clone(),
            sampled_gas_price: sample.sampled_gas_price.clone(),
            proposed_gas_price: proposal.proposed.gas_price.clone(),
            rounding: sample.rounding.clone(),
            reason: sample.reason.clone(),
            endpoint: sample.endpoint.clone(),
        },
        None => GasInputsArtifact {
            mode: proposal.gas_mode.clone(),
            source: None,
            raw_amount: None,
            raw_denom: None,
            sampled_gas_price: proposal.proposed.gas_price.clone(),
            proposed_gas_price: proposal.proposed.gas_price.clone(),
            rounding: None,
            reason: None,
            endpoint: None,
        },
    }
}

fn price_inputs(
    target: &ReconciliationTarget,
    config: &UpdaterConfig,
    proposal: &ProposalComputation,
) -> PriceInputsArtifact {
    PriceInputsArtifact {
        price_provider: config.market_data.provider.clone(),
        origin_market_asset: config.market_data.assets.get(&target.origin.name).cloned(),
        remote_market_asset: config.market_data.assets.get(&target.remote.name).cloned(),
        origin_price_usd: proposal.origin_price_usd.clone(),
        remote_price_usd: proposal.remote_price_usd.clone(),
        origin_native_token_decimals: proposal.origin_native_token_decimals,
        remote_native_token_decimals: proposal.remote_native_token_decimals,
        token_decimal_adjustment: proposal.token_decimal_adjustment.clone(),
    }
}

pub fn write_artifacts(output_dir: &Path, plan: &PlanArtifact) -> Result<()> {
    create_dir_all(output_dir.to_path_buf())?;

    let summary_path = output_dir.join("igp-summary.md");
    write(&summary_path, render_summary(plan))?;

    write_json(output_dir.join("igp-plan.json"), plan)?;
    remove_legacy_tx_plan(output_dir)?;

    Ok(())
}

fn remove_legacy_tx_plan(output_dir: &Path) -> Result<()> {
    let path = output_dir.join("tx-plan.json");
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(IgpOracleError::Io { path, source }),
    }
}

fn write_json(path: PathBuf, value: &impl Serialize) -> Result<()> {
    let json = serde_json::to_string_pretty(value).map_err(|source| IgpOracleError::Json {
        path: path.clone(),
        source,
    })?;
    write(path, format!("{json}\n"))
}

pub fn render_summary(plan: &PlanArtifact) -> String {
    let mut out = String::new();
    out.push_str("# IGP Oracle Dry Run\n\n");
    out.push_str(&format!(
        "- Git SHA: {}\n",
        plan.git_sha.as_deref().unwrap_or("unknown")
    ));
    out.push_str(&format!("- Targets: {}\n\n", plan.targets.len()));
    if !plan.discovery.is_empty() {
        let discovered: usize = plan
            .discovery
            .iter()
            .map(|discovery| discovery.configured_remote_domains)
            .sum();
        let resolved: usize = plan
            .discovery
            .iter()
            .map(|discovery| discovery.resolved_remote_domains)
            .sum();
        let skipped: usize = plan
            .discovery
            .iter()
            .map(|discovery| discovery.skipped_remote_domains)
            .sum();
        out.push_str(&format!(
            "- Discovered domains: {discovered}\n- Resolved domains: {resolved}\n- Skipped domains: {skipped}\n\n"
        ));
    }
    out.push_str("| Origin | Remote | Domain | IGP | Gas price | Exchange rate | Gas overhead | Decision | Code | Driver |\n");
    out.push_str("| --- | --- | ---: | --- | ---: | ---: | ---: | --- | --- | --- |\n");

    for target in &plan.targets {
        let gas_price = target
            .proposed
            .as_ref()
            .map(|proposed| proposed.gas_price.as_str())
            .unwrap_or("n/a");
        let exchange_rate = target
            .proposed
            .as_ref()
            .map(|proposed| proposed.token_exchange_rate.as_str())
            .unwrap_or("n/a");
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            target.origin_chain,
            target.remote_chain,
            target.remote_domain,
            target.igp_identifier.as_deref().unwrap_or("n/a"),
            gas_price,
            exchange_rate,
            target.gas_overhead,
            target.decision.status.as_str(),
            target
                .decision
                .code
                .map(DecisionCode::as_str)
                .unwrap_or("n/a"),
            target
                .decision
                .field
                .map(ReconciliationField::as_str)
                .unwrap_or("n/a")
        ));
    }

    out
}
