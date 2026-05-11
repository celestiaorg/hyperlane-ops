use async_trait::async_trait;
use celestia_grpc::{GrpcClient, TxConfig};

use crate::{
    adapter::ChainAdapter,
    config::SignerConfig,
    cosmosnative::query::CosmosNativeQueryClient,
    error::{IgpOracleError, Result},
    models::{
        ChainMetadata, ChainProtocol, ConfiguredRemoteDomain, CoreAddresses, IgpConfigRead,
        ProposedIgpConfig, ReconciliationTarget, TxPlan, TxReceipt, TxSigner, VerificationResult,
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
                "review artifact; submit mode signs and broadcasts this protobuf message through celestia-grpc".to_string(),
                "values are recomputed immediately before any write submission".to_string(),
            ],
        })
    }

    async fn submit_update(
        &self,
        target: &ReconciliationTarget,
        plan: &TxPlan,
        signer: &SignerConfig,
    ) -> Result<TxReceipt> {
        if target.config.write.method != "celestia-grpc" {
            return Err(IgpOracleError::InvalidConfig(format!(
                "cosmosnative write method must be celestia-grpc, got {}",
                target.config.write.method
            )));
        }

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
        let private_key = private_key_from_env(signer)?;
        let message = msg_set_destination_gas_config(plan)?;
        let client = GrpcClient::builder()
            .url(endpoint)
            .private_key_hex(&private_key)
            .build()
            .map_err(|source| {
                IgpOracleError::DataSource(format!(
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
                IgpOracleError::DataSource(format!(
                    "failed to submit cosmosnative IGP update for {} -> {}: {source}",
                    target.origin.name, target.remote.name
                ))
            })?;

        Ok(TxReceipt {
            tx_hash: tx_info.hash.to_string(),
            height: Some(tx_info.height),
        })
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

fn private_key_from_env(signer: &SignerConfig) -> Result<String> {
    let value = std::env::var(&signer.key_env).map_err(|_| {
        IgpOracleError::InvalidConfig(format!("signer key env {} is not set", signer.key_env))
    })?;
    let value = value.trim();
    let value = value.strip_prefix("0x").unwrap_or(value);
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(IgpOracleError::InvalidConfig(format!(
            "signer key env {} must contain a 32-byte hex private key",
            signer.key_env
        )));
    }

    Ok(value.to_string())
}

fn msg_set_destination_gas_config(plan: &TxPlan) -> Result<MsgSetDestinationGasConfig> {
    if plan.message_type != "/hyperlane.core.post_dispatch.v1.MsgSetDestinationGasConfig" {
        return Err(IgpOracleError::UnsupportedWrite);
    }

    let destination = plan
        .message
        .get("destinationGasConfig")
        .ok_or_else(|| missing_plan_field("destinationGasConfig"))?;
    let gas_oracle = destination
        .get("gasOracle")
        .ok_or_else(|| missing_plan_field("destinationGasConfig.gasOracle"))?;

    Ok(MsgSetDestinationGasConfig {
        owner: required_str(&plan.message, "owner")?.to_string(),
        igp_id: required_str(&plan.message, "igpId")?.to_string(),
        destination_gas_config: Some(DestinationGasConfig {
            remote_domain: required_u32(destination, "remoteDomain")?,
            gas_oracle: Some(GasOracle {
                token_exchange_rate: required_str(gas_oracle, "tokenExchangeRate")?.to_string(),
                gas_price: required_str(gas_oracle, "gasPrice")?.to_string(),
            }),
            gas_overhead: required_str(destination, "gasOverhead")?.to_string(),
        }),
    })
}

fn required_str<'a>(value: &'a serde_json::Value, field: &str) -> Result<&'a str> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| missing_plan_field(field))
}

fn required_u32(value: &serde_json::Value, field: &str) -> Result<u32> {
    let value = value
        .get(field)
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| missing_plan_field(field))?;
    u32::try_from(value).map_err(|_| {
        IgpOracleError::InvalidTarget(format!(
            "cosmosnative tx plan field {field} does not fit u32: {value}"
        ))
    })
}

fn missing_plan_field(field: &str) -> IgpOracleError {
    IgpOracleError::InvalidTarget(format!(
        "cosmosnative tx plan is missing required field {field}"
    ))
}
