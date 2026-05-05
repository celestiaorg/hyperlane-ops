use async_trait::async_trait;
use rust_decimal::Decimal;

use crate::{
    cosmosnative::query::CosmosNativeQueryClient,
    error::{IgpOracleError, Result},
    evm::query::EvmIgpReader,
    models::{
        ChainProtocol, IgpConfigRead, ProposedIgpConfig, ReconciliationTarget, TxPlan, TxReceipt,
        TxSigner, VerificationResult,
    },
};

#[async_trait]
pub trait ChainAdapter: Send + Sync {
    fn protocol(&self) -> ChainProtocol;

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
    async fn remote_gas_price(&self, target: &ReconciliationTarget) -> Result<u128>;
}

#[async_trait]
pub trait PriceAdapter: Send + Sync {
    async fn native_token_price_usd(&self, chain_name: &str) -> Result<Decimal>;
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

    async fn read_igp_config(&self, target: &ReconciliationTarget) -> Result<IgpConfigRead> {
        let igp_id = target
            .origin_addresses
            .interchain_gas_paymaster
            .as_deref()
            .ok_or_else(|| {
                IgpOracleError::Registry(format!(
                    "origin chain {} has no interchainGasPaymaster address",
                    target.origin.name
                ))
            })?;
        let endpoint = target
            .origin
            .grpc_urls
            .first()
            .ok_or_else(|| {
                IgpOracleError::DataSource(format!(
                    "origin chain {} has no grpcUrls entry",
                    target.origin.name
                ))
            })?
            .http
            .as_str();

        let (config, source) = CosmosNativeQueryClient::new(endpoint)
            .destination_gas_config(&target.origin.name, igp_id, target.remote.domain_id)
            .await?;

        Ok(IgpConfigRead { config, source })
    }

    async fn plan_update(
        &self,
        target: &ReconciliationTarget,
        proposed: &ProposedIgpConfig,
    ) -> Result<TxPlan> {
        let igp_id = target
            .origin_addresses
            .interchain_gas_paymaster
            .as_deref()
            .ok_or_else(|| {
                IgpOracleError::Registry(format!(
                    "origin chain {} has no interchainGasPaymaster address",
                    target.origin.name
                ))
            })?;
        let endpoint = target
            .origin
            .grpc_urls
            .first()
            .ok_or_else(|| {
                IgpOracleError::DataSource(format!(
                    "origin chain {} has no grpcUrls entry",
                    target.origin.name
                ))
            })?
            .http
            .as_str();
        let owner = CosmosNativeQueryClient::new(endpoint)
            .igp_owner(&target.origin.name, igp_id)
            .await?;

        Ok(TxPlan {
            protocol: "cosmosnative".to_string(),
            action: "setDestinationGasConfig".to_string(),
            message_type: "/hyperlane.core.post_dispatch.v1.MsgSetDestinationGasConfig"
                .to_string(),
            target: igp_id.to_string(),
            selector: None,
            calldata: None,
            command: None,
            signer: Some(TxSigner {
                signer_profile: target.config.write.signer_profile.clone(),
                address: Some(owner.clone()),
            }),
            message: serde_json::json!({
                "owner": owner,
                "igpId": igp_id,
                "destinationGasConfig": {
                    "remoteDomain": target.remote.domain_id,
                    "gasOracle": {
                        "tokenExchangeRate": proposed.token_exchange_rate.as_str(),
                        "gasPrice": proposed.gas_price.as_str()
                    },
                    "gasOverhead": proposed.gas_overhead.to_string()
                }
            }),
            notes: vec![
                "review artifact only; transaction submission is not implemented".to_string(),
                "values were generated from the dry-run proposal and must be recomputed before write mode".to_string(),
            ],
        })
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
