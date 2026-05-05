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
    pub configured_gas_overhead: u64,
    pub policy: PolicyArtifact,
    pub inputs: Option<ProposalInputsArtifact>,
    pub on_chain_read: Option<OnChainReadArtifact>,
    pub current: Option<CurrentIgpConfig>,
    pub proposed: Option<ProposedIgpConfig>,
    pub deltas: Option<ReconciliationDelta>,
    pub tx: Option<TxPlan>,
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
    pub max_delta_bps: Option<u128>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposalInputsArtifact {
    pub origin_price_usd: String,
    pub remote_price_usd: String,
    pub remote_gas_price: String,
    pub price_provider: String,
    pub origin_market_asset: Option<String>,
    pub remote_market_asset: Option<String>,
    pub gas_source: String,
    pub remote_gas_endpoint: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnChainReadArtifact {
    pub protocol: String,
    pub endpoint: Option<String>,
    pub query: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TxPlanArtifact {
    pub git_sha: Option<String>,
    pub plans: Vec<TxPlanEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TxPlanEntry {
    pub origin_chain: String,
    pub remote_chain: String,
    pub remote_domain: u32,
    pub decision: DecisionArtifact,
    pub tx: Option<TxPlan>,
}

pub struct TargetArtifactInput<'a> {
    pub target: &'a ReconciliationTarget,
    pub config: &'a UpdaterConfig,
    pub policy: PolicyArtifact,
    pub proposal: Option<ProposalComputation>,
    pub current_read: Option<IgpConfigRead>,
    pub deltas: Option<ReconciliationDelta>,
    pub tx: Option<TxPlan>,
    pub decision: DecisionArtifact,
}

pub fn target_artifact(input: TargetArtifactInput<'_>) -> TargetPlanArtifact {
    let inputs = input
        .proposal
        .as_ref()
        .map(|proposal| proposal_inputs(input.target, input.config, proposal));
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
        configured_gas_overhead: input.target.config.gas_overhead,
        policy: input.policy,
        inputs,
        on_chain_read,
        current,
        proposed,
        deltas: input.deltas,
        tx: input.tx,
        decision: input.decision,
    }
}

fn proposal_inputs(
    target: &ReconciliationTarget,
    config: &UpdaterConfig,
    proposal: &ProposalComputation,
) -> ProposalInputsArtifact {
    ProposalInputsArtifact {
        origin_price_usd: proposal.origin_price_usd.clone(),
        remote_price_usd: proposal.remote_price_usd.clone(),
        remote_gas_price: proposal.remote_gas_price.clone(),
        price_provider: config.market_data.provider.clone(),
        origin_market_asset: config.market_data.assets.get(&target.origin.name).cloned(),
        remote_market_asset: config.market_data.assets.get(&target.remote.name).cloned(),
        gas_source: target.config.gas.source.clone(),
        remote_gas_endpoint: target
            .remote
            .rpc_urls
            .first()
            .map(|entry| entry.http.clone())
            .or_else(|| {
                target.remote.gas_price.as_ref().map(|gas_price| {
                    format!("registry gasPrice {}{}", gas_price.amount, gas_price.denom)
                })
            }),
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

pub fn tx_plan_entry(target: &TargetPlanArtifact) -> TxPlanEntry {
    TxPlanEntry {
        origin_chain: target.origin_chain.clone(),
        remote_chain: target.remote_chain.clone(),
        remote_domain: target.remote_domain,
        decision: target.decision.clone(),
        tx: target.tx.clone(),
    }
}

pub fn write_artifacts(output_dir: &Path, plan: &PlanArtifact) -> Result<()> {
    create_dir_all(output_dir.to_path_buf())?;

    let summary_path = output_dir.join("igp-summary.md");
    write(&summary_path, render_summary(plan))?;

    write_json(output_dir.join("igp-plan.json"), plan)?;

    let tx_artifact = TxPlanArtifact {
        git_sha: plan.git_sha.clone(),
        plans: plan.targets.iter().map(tx_plan_entry).collect(),
    };
    write_json(output_dir.join("tx-plan.json"), &tx_artifact)?;

    Ok(())
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
            target.configured_gas_overhead,
            target.decision.status,
            target.decision.code,
            target.decision.field.as_deref().unwrap_or("n/a")
        ));
    }

    out
}
