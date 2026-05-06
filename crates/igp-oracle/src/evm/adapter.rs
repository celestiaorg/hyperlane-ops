use async_trait::async_trait;

use crate::{
    adapter::ChainAdapter,
    error::{IgpOracleError, Result},
    evm::query::EvmIgpReader,
    models::{
        ChainMetadata, ChainProtocol, ConfiguredRemoteDomain, CoreAddresses, IgpConfigRead,
        ProposedIgpConfig, ReconciliationTarget, TxPlan, TxReceipt, VerificationResult,
    },
};

#[derive(Debug, Default)]
pub struct EvmAdapter;

#[async_trait]
impl ChainAdapter for EvmAdapter {
    fn protocol(&self) -> ChainProtocol {
        ChainProtocol::Ethereum
    }

    async fn list_igp_destination_configs(
        &self,
        origin: &ChainMetadata,
        _origin_addresses: &CoreAddresses,
    ) -> Result<Vec<ConfiguredRemoteDomain>> {
        Err(IgpOracleError::UnsupportedLiveRead(format!(
            "EVM IGP destination config discovery is unsupported for origin {}",
            origin.name
        )))
    }

    async fn read_igp_config(&self, target: &ReconciliationTarget) -> Result<IgpConfigRead> {
        EvmIgpReader::new()?.read_igp_config(target).await
    }

    async fn plan_update(
        &self,
        _target: &ReconciliationTarget,
        _proposed: &ProposedIgpConfig,
    ) -> Result<TxPlan> {
        Err(IgpOracleError::UnsupportedLiveRead(
            "EVM tx planning".to_string(),
        ))
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
            "EVM verification".to_string(),
        ))
    }
}
