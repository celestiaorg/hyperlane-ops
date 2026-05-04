use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::{
    config::DefaultsConfig,
    error::{create_dir_all, write, IgpOracleError, Result},
    models::{CurrentIgpConfig, ProposedIgpConfig, ReconciliationTarget, TxPlan},
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
    pub current: Option<CurrentIgpConfig>,
    pub proposed: Option<ProposedIgpConfig>,
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
    pub reason: String,
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

pub fn target_artifact(
    target: &ReconciliationTarget,
    policy: PolicyArtifact,
    proposed: Option<ProposedIgpConfig>,
    decision: DecisionArtifact,
) -> TargetPlanArtifact {
    TargetPlanArtifact {
        origin_chain: target.origin.name.clone(),
        remote_chain: target.remote.name.clone(),
        remote_domain: target.remote.domain_id,
        origin_protocol: target.origin.protocol.as_str().to_string(),
        remote_protocol: target.remote.protocol.as_str().to_string(),
        igp_identifier: target.origin_addresses.interchain_gas_paymaster.clone(),
        configured_gas_overhead: target.config.gas_overhead,
        policy,
        current: None,
        proposed,
        tx: None,
        decision,
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
    out.push_str("| Origin | Remote | Domain | IGP | Gas price | Exchange rate | Gas overhead | Decision |\n");
    out.push_str("| --- | --- | ---: | --- | ---: | ---: | ---: | --- |\n");

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
            "| {} | {} | {} | {} | {} | {} | {} | {} |\n",
            target.origin_chain,
            target.remote_chain,
            target.remote_domain,
            target.igp_identifier.as_deref().unwrap_or("n/a"),
            gas_price,
            exchange_rate,
            target.configured_gas_overhead,
            target.decision.status
        ));
    }

    out
}
