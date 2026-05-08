use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::{
    config::{DefaultsConfig, UpdaterConfig},
    error::{create_dir_all, write, IgpOracleError, Result},
    models::{
        CurrentIgpConfig, IgpConfigRead, ProposalComputation, ProposedIgpConfig,
        ReconciliationDelta, ReconciliationTarget, TxPlan,
    },
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanArtifact {
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
    pub origin_protocol: String,
    pub remote_protocol: String,
    pub igp_identifier: Option<String>,
    pub write_enabled: bool,
    pub write_method: String,
    pub gas_overhead: u64,
    pub gas: Option<GasInputsArtifact>,
    pub prices: Option<PriceInputsArtifact>,
    pub on_chain_read: Option<OnChainReadArtifact>,
    pub current: Option<CurrentIgpConfig>,
    pub proposed: Option<ProposedIgpConfig>,
    pub deltas: Option<ReconciliationDelta>,
    pub tx: Option<TxPlan>,
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
    pub status: String,
    pub code: String,
    pub field: Option<String>,
    pub delta_bps: Option<u128>,
    pub min_write_delta_bps: Option<u64>,
    pub max_allowed_delta_bps: Option<u64>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TxPlanErrorArtifact {
    pub code: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WritePlanArtifact {
    pub mode: String,
    pub status: String,
    pub protocol: String,
    pub origin_chain: String,
    pub transaction_model: String,
    pub target_count: usize,
    pub message_count: usize,
    pub targets: Vec<WritePlanTargetArtifact>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WritePlanTargetArtifact {
    pub remote_chain: String,
    pub remote_domain: u32,
    pub action: String,
    pub target: String,
    pub selector: Option<String>,
    pub signer_authorization: SignerAuthorizationArtifact,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignerAuthorizationArtifact {
    pub signer_profile: String,
    pub configured_signer: String,
    pub authorized_signer: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryArtifact {
    pub origin_chain: String,
    pub igp_identifier: Option<String>,
    pub protocol: String,
    pub configured_remote_domains: usize,
    pub resolved_remote_domains: usize,
    pub skipped_remote_domains: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedTargetArtifact {
    pub origin_chain: String,
    pub remote_domain: u32,
    pub status: String,
    pub code: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GasInputsArtifact {
    pub mode: String,
    pub source: String,
    pub remote_chain: String,
    pub raw_amount: Option<String>,
    pub raw_denom: Option<String>,
    pub sampled_gas_price: String,
    pub proposed_gas_price: String,
    pub rounding: Option<String>,
    pub reason: Option<String>,
    pub endpoint: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceInputsArtifact {
    pub price_provider: String,
    pub origin_market_asset: Option<String>,
    pub remote_market_asset: Option<String>,
    pub origin_price_usd: String,
    pub remote_price_usd: String,
    pub origin_native_token_decimals: u8,
    pub remote_native_token_decimals: u8,
    pub token_decimal_adjustment: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnChainReadArtifact {
    pub protocol: String,
    pub endpoint: Option<String>,
    pub query: String,
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
    let gas = input
        .proposal
        .as_ref()
        .map(|proposal| gas_inputs(input.target, proposal));
    let prices = input
        .proposal
        .as_ref()
        .map(|proposal| price_inputs(input.target, input.config, proposal));
    let on_chain_read = input
        .current_read
        .as_ref()
        .map(|read| OnChainReadArtifact::from(&read.source));
    let current = input.current_read.map(|read| read.config);
    let proposed = input.proposal.map(|proposal| proposal.proposed);

    TargetPlanArtifact {
        origin_chain: input.target.origin.name.clone(),
        remote_chain: input.target.remote.name.clone(),
        remote_domain: input.target.remote.domain_id,
        origin_protocol: input.target.origin.protocol.as_str().to_string(),
        remote_protocol: input.target.remote.protocol.as_str().to_string(),
        igp_identifier: input
            .target
            .origin_addresses
            .interchain_gas_paymaster
            .clone(),
        write_enabled: input.target.config.write.enabled,
        write_method: input.target.config.write.method.clone(),
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

fn gas_inputs(target: &ReconciliationTarget, proposal: &ProposalComputation) -> GasInputsArtifact {
    match proposal.gas.as_ref() {
        Some(sample) => GasInputsArtifact {
            mode: proposal.gas_mode.clone(),
            source: sample.source.clone(),
            remote_chain: sample.remote_chain.clone(),
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
            source: "onchain".to_string(),
            remote_chain: target.remote.name.clone(),
            raw_amount: None,
            raw_denom: None,
            sampled_gas_price: proposal.proposed.gas_price.clone(),
            proposed_gas_price: proposal.proposed.gas_price.clone(),
            rounding: None,
            reason: Some("gasPrice preserved from current on-chain config".to_string()),
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

impl From<&crate::models::OnChainReadSource> for OnChainReadArtifact {
    fn from(source: &crate::models::OnChainReadSource) -> Self {
        Self {
            protocol: source.protocol.clone(),
            endpoint: source.endpoint.clone(),
            query: source.query.clone(),
        }
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
            target.decision.status,
            target.decision.code,
            target.decision.field.as_deref().unwrap_or("n/a")
        ));
    }

    out
}
