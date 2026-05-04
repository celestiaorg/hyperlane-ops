use std::{path::Path, process::Command};

use crate::{
    adapters::adapter_for,
    artifacts::{target_artifact, write_artifacts, DecisionArtifact, PlanArtifact, PolicyArtifact},
    cli::ReconcileArgs,
    config::UpdaterConfig,
    error::{IgpOracleError, Result},
    registry::RegistryLoader,
    resolver::resolve_targets,
};

pub async fn run_reconcile(args: ReconcileArgs) -> Result<i32> {
    if args.write {
        return Err(IgpOracleError::UnsupportedWrite);
    }

    let config = UpdaterConfig::load(&args.config)?;
    let registry = RegistryLoader::new(&args.registry);
    let targets = resolve_targets(&config, &registry, &args)?;

    let mut target_artifacts = Vec::new();
    for target in targets {
        let adapter = adapter_for(target.origin.protocol);
        let decision = match adapter.read_igp_config(&target).await {
            Ok(_) => DecisionArtifact {
                status: "not_evaluated".to_string(),
                reason: "stage one does not compute live proposed values".to_string(),
            },
            Err(IgpOracleError::UnsupportedLiveRead(reason)) => DecisionArtifact {
                status: "unsupported_live_read".to_string(),
                reason,
            },
            Err(err) => return Err(err),
        };

        target_artifacts.push(target_artifact(
            &target,
            PolicyArtifact::from(&config.defaults),
            decision,
        ));
    }

    let plan = PlanArtifact {
        git_sha: git_sha(&args.registry),
        targets: target_artifacts,
    };

    write_artifacts(&args.output_dir, &plan)?;
    Ok(0)
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
    use std::path::PathBuf;

    use tempfile::tempdir;

    use crate::cli::ReconcileArgs;

    use super::*;

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

        let code = run_reconcile(args).await.expect("dry-run should succeed");
        assert_eq!(code, 0);
        assert!(output_dir.path().join("igp-summary.md").exists());
        assert!(output_dir.path().join("igp-plan.json").exists());
        assert!(output_dir.path().join("tx-plan.json").exists());

        let plan = std::fs::read_to_string(output_dir.path().join("igp-plan.json")).expect("plan");
        assert!(plan.contains("\"status\": \"unsupported_live_read\""));
        assert!(plan.contains("\"current\": null"));
        assert!(plan.contains("\"proposed\": null"));
        assert!(plan.contains("\"tx\": null"));
    }
}
