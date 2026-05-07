use async_trait::async_trait;
use rust_decimal::Decimal;

use crate::{
    cosmosnative::adapter::CosmosNativeAdapter,
    error::Result,
    evm::adapter::EvmAdapter,
    models::{
        ChainMetadata, ChainProtocol, ConfiguredRemoteDomain, CoreAddresses, GasPriceSample,
        IgpConfigRead, ProposedIgpConfig, ReconciliationTarget, TxPlan, TxReceipt,
        VerificationResult,
    },
};

#[async_trait]
pub trait ChainAdapter: Send + Sync {
    fn protocol(&self) -> ChainProtocol;

    async fn list_igp_destination_configs(
        &self,
        origin: &ChainMetadata,
        origin_addresses: &CoreAddresses,
    ) -> Result<Vec<ConfiguredRemoteDomain>>;

    async fn read_igp_config(&self, target: &ReconciliationTarget) -> Result<IgpConfigRead>;

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
    async fn remote_gas_price(&self, target: &ReconciliationTarget) -> Result<GasPriceSample>;
}

#[async_trait]
pub trait PriceAdapter: Send + Sync {
    async fn native_token_price_usd(&self, chain_name: &str) -> Result<Decimal>;
}

pub fn adapter_for(protocol: ChainProtocol) -> Box<dyn ChainAdapter> {
    match protocol {
        ChainProtocol::CosmosNative => Box::<CosmosNativeAdapter>::default(),
        ChainProtocol::Ethereum => Box::<EvmAdapter>::default(),
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
