use async_trait::async_trait;

use crate::{
    adapter::ChainAdapter,
    cosmosnative::query::CosmosNativeQueryClient,
    error::{IgpOracleError, Result},
    models::{
        ChainMetadata, ChainProtocol, ConfiguredRemoteDomain, CoreAddresses, IgpConfigRead,
        ProposedIgpConfig, ReconciliationTarget, TxPlan, TxReceipt, TxSigner, VerificationResult,
    },
};

#[derive(Debug, Default)]
pub struct CosmosNativeAdapter;

#[async_trait]
impl ChainAdapter for CosmosNativeAdapter {
    fn protocol(&self) -> ChainProtocol {
        ChainProtocol::CosmosNative
    }

    async fn list_igp_destination_configs(
        &self,
        origin: &ChainMetadata,
        origin_addresses: &CoreAddresses,
    ) -> Result<Vec<ConfiguredRemoteDomain>> {
        let igp_id = origin_addresses
            .interchain_gas_paymaster
            .as_deref()
            .ok_or_else(|| {
                IgpOracleError::Registry(format!(
                    "origin chain {} has no interchainGasPaymaster address",
                    origin.name
                ))
            })?;
        let endpoint = origin
            .grpc_urls
            .first()
            .ok_or_else(|| {
                IgpOracleError::DataSource(format!(
                    "origin chain {} has no grpcUrls entry",
                    origin.name
                ))
            })?
            .http
            .as_str();

        CosmosNativeQueryClient::new(endpoint)
            .destination_gas_configs(&origin.name, igp_id)
            .await
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
