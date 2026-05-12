use async_trait::async_trait;
use celestia_grpc::{GrpcClient, TxConfig};

use crate::{
    adapter::{ChainAdapter, SignerAuthStatus},
    config::{SignerConfig, WriteMethod},
    cosmosnative::query::CosmosNativeQueryClient,
    error::{IgpOracleError, Result},
    models::{
        ChainMetadata, ChainProtocol, ConfiguredRemoteDomain, CoreAddresses, IgpConfig,
        IgpConfigRead, ReconciliationTarget, TxPayload, TxPlan, TxReceipt, TxSigner,
        VerificationResult,
    },
    proto::hyperlane::core::post_dispatch::v1::{
        DestinationGasConfig, GasOracle, MsgSetDestinationGasConfig,
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
                IgpOracleError::OnchainRead(format!(
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
                IgpOracleError::OnchainRead(format!(
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
        proposed: &IgpConfig,
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
                IgpOracleError::OnchainRead(format!(
                    "origin chain {} has no grpcUrls entry",
                    target.origin.name
                ))
            })?
            .http
            .as_str();
        let owner = CosmosNativeQueryClient::new(endpoint)
            .igp_owner(&target.origin.name, igp_id)
            .await?;

        let message = MsgSetDestinationGasConfig {
            owner: owner.clone(),
            igp_id: igp_id.to_string(),
            destination_gas_config: Some(DestinationGasConfig {
                remote_domain: target.remote.domain_id,
                gas_oracle: Some(GasOracle {
                    token_exchange_rate: proposed.token_exchange_rate.clone(),
                    gas_price: proposed.gas_price.clone(),
                }),
                gas_overhead: proposed.gas_overhead.to_string(),
            }),
        };

        Ok(TxPlan {
            protocol: ChainProtocol::CosmosNative,
            action: "setDestinationGasConfig".to_string(),
            message_type: "/hyperlane.core.post_dispatch.v1.MsgSetDestinationGasConfig"
                .to_string(),
            target: igp_id.to_string(),
            selector: None,
            calldata: None,
            signer: Some(TxSigner {
                signer_profile: target.config.write.signer_profile.clone(),
                address: Some(owner),
            }),
            payload: TxPayload::CosmosSetDestinationGasConfig(message),
        })
    }

    async fn submit_update(
        &self,
        target: &ReconciliationTarget,
        plan: &TxPlan,
        signer: &SignerConfig,
    ) -> Result<TxReceipt> {
        if target.config.write.method != WriteMethod::CelestiaGrpc {
            return Err(IgpOracleError::InvalidConfig(
                "cosmosnative write method must be celestia-grpc".to_string(),
            ));
        }

        let TxPayload::CosmosSetDestinationGasConfig(message) = plan.payload.clone() else {
            return Err(IgpOracleError::UnsupportedWrite);
        };

        let endpoint = target
            .origin
            .grpc_urls
            .first()
            .ok_or_else(|| {
                IgpOracleError::OnchainRead(format!(
                    "origin chain {} has no grpcUrls entry",
                    target.origin.name
                ))
            })?
            .http
            .as_str();
        let private_key = signer.load_private_key_hex()?;
        let client = GrpcClient::builder()
            .url(endpoint)
            .private_key_hex(&private_key)
            .build()
            .map_err(|source| {
                IgpOracleError::OnchainRead(format!(
                    "failed to build celestia-grpc client for {}: {source}",
                    target.origin.name
                ))
            })?;

        let tx_info = client
            .submit_message(
                message,
                TxConfig::default().with_memo("igp-oracle setDestinationGasConfig"),
            )
            .await
            .map_err(|source| {
                IgpOracleError::OnchainRead(format!(
                    "failed to submit cosmosnative IGP update for {} -> {}: {source}",
                    target.origin.name, target.remote.name
                ))
            })?;

        Ok(TxReceipt {
            tx_hash: tx_info.hash.to_string(),
            height: Some(tx_info.height),
        })
    }

    fn check_signer_authorization(
        &self,
        tx_signer: &TxSigner,
        signer_config: &SignerConfig,
    ) -> Result<SignerAuthStatus> {
        let Some(authorized) = tx_signer.address.as_deref() else {
            return Ok(SignerAuthStatus::AuthorityUnavailable);
        };
        if !is_probable_cosmos_address(&signer_config.from) {
            return Ok(SignerAuthStatus::KeyAliasUnverified);
        }
        if signer_config.from != authorized {
            return Err(IgpOracleError::InvalidTarget(format!(
                "configured signer {} is not authorized for cosmosnative target; expected {authorized}",
                signer_config.from
            )));
        }
        Ok(SignerAuthStatus::AddressMatch)
    }

    async fn verify_update(
        &self,
        _target: &ReconciliationTarget,
        _expected: &IgpConfig,
    ) -> Result<VerificationResult> {
        Err(IgpOracleError::UnsupportedLiveRead(
            "cosmosnative verification".to_string(),
        ))
    }
}

fn is_probable_cosmos_address(value: &str) -> bool {
    value.len() > 20 && value.contains('1')
}

