use async_trait::async_trait;

use crate::{
    error::{IgpOracleError, Result},
    models::{
        ChainProtocol, CurrentIgpConfig, ProposedIgpConfig, ReconciliationTarget, TxPlan,
        TxReceipt, VerificationResult,
    },
};

#[async_trait]
pub trait ChainAdapter: Send + Sync {
    fn protocol(&self) -> ChainProtocol;

    async fn read_igp_config(&self, target: &ReconciliationTarget) -> Result<CurrentIgpConfig>;

    async fn plan_update(
        &self,
        target: &ReconciliationTarget,
        proposed: &ProposedIgpConfig,
    ) -> Result<TxPlan>;

    async fn submit_update(
        &self,
        target: &ReconciliationTarget,
        plan: &TxPlan,
    ) -> Result<TxReceipt>;

    async fn verify_update(
        &self,
        target: &ReconciliationTarget,
        expected: &ProposedIgpConfig,
    ) -> Result<VerificationResult>;
}

#[async_trait]
pub trait GasAdapter: Send + Sync {
    async fn remote_gas_price(&self, target: &ReconciliationTarget) -> Result<String>;
}

#[async_trait]
pub trait PriceAdapter: Send + Sync {
    async fn native_token_price_usd(&self, chain_name: &str) -> Result<String>;
}

#[derive(Debug, Default)]
pub struct CosmosNativeAdapter;

#[derive(Debug, Default)]
pub struct EvmAdapter;

pub fn adapter_for(protocol: ChainProtocol) -> Box<dyn ChainAdapter> {
    match protocol {
        ChainProtocol::CosmosNative => Box::<CosmosNativeAdapter>::default(),
        ChainProtocol::Ethereum => Box::<EvmAdapter>::default(),
    }
}

#[async_trait]
impl ChainAdapter for CosmosNativeAdapter {
    fn protocol(&self) -> ChainProtocol {
        ChainProtocol::CosmosNative
    }

    async fn read_igp_config(&self, target: &ReconciliationTarget) -> Result<CurrentIgpConfig> {
        Err(IgpOracleError::UnsupportedLiveRead(format!(
            "cosmosnative IGP reads for origin {}",
            target.origin.name
        )))
    }

    async fn plan_update(
        &self,
        _target: &ReconciliationTarget,
        _proposed: &ProposedIgpConfig,
    ) -> Result<TxPlan> {
        Err(IgpOracleError::UnsupportedLiveRead(
            "cosmosnative tx planning".to_string(),
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
            "cosmosnative verification".to_string(),
        ))
    }
}

#[async_trait]
impl ChainAdapter for EvmAdapter {
    fn protocol(&self) -> ChainProtocol {
        ChainProtocol::Ethereum
    }

    async fn read_igp_config(&self, target: &ReconciliationTarget) -> Result<CurrentIgpConfig> {
        Err(IgpOracleError::UnsupportedLiveRead(format!(
            "EVM IGP reads for origin {}",
            target.origin.name
        )))
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

#[cfg(test)]
mod tests {
    use crate::models::ChainProtocol;

    use super::*;

    #[test]
    fn adapter_factory_returns_cosmosnative_adapter() {
        let adapter = adapter_for(ChainProtocol::CosmosNative);
        assert_eq!(adapter.protocol(), ChainProtocol::CosmosNative);
    }

    #[test]
    fn adapter_factory_returns_evm_adapter() {
        let adapter = adapter_for(ChainProtocol::Ethereum);
        assert_eq!(adapter.protocol(), ChainProtocol::Ethereum);
    }
}
