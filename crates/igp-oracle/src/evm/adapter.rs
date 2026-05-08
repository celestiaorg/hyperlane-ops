use async_trait::async_trait;

use crate::{
    adapter::ChainAdapter,
    error::{IgpOracleError, Result},
    evm::query::{encode_set_remote_gas_data, EvmIgpReader, SET_REMOTE_GAS_DATA},
    models::{
        ChainMetadata, ChainProtocol, ConfiguredRemoteDomain, CoreAddresses, IgpConfigRead,
        ProposedIgpConfig, ReconciliationTarget, TxPlan, TxReceipt, TxSigner, VerificationResult,
    },
};

#[derive(Debug, Default)]
pub struct EvmAdapter;

#[async_trait]
impl ChainAdapter for EvmAdapter {
    fn protocol(&self) -> ChainProtocol {
        ChainProtocol::Ethereum
    }

    fn supports_destination_config_discovery(&self) -> bool {
        false
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
        target: &ReconciliationTarget,
        proposed: &ProposedIgpConfig,
    ) -> Result<TxPlan> {
        let reader = EvmIgpReader::new()?;
        let destination_config = reader.read_destination_gas_config(target).await?;

        if proposed.gas_overhead != destination_config.gas_overhead {
            return Err(IgpOracleError::UnsupportedLiveRead(format!(
                "EVM gasOverhead tx planning is not implemented; current={} proposed={}",
                destination_config.gas_overhead, proposed.gas_overhead
            )));
        }

        let token_exchange_rate =
            parse_u128_config("tokenExchangeRate", &proposed.token_exchange_rate)?;
        let gas_price = parse_u128_config("gasPrice", &proposed.gas_price)?;
        let calldata =
            encode_set_remote_gas_data(target.remote.domain_id, token_exchange_rate, gas_price);
        let owner = reader
            .read_owner(&destination_config.endpoint, &destination_config.gas_oracle)
            .await?;

        Ok(TxPlan {
            protocol: "ethereum".to_string(),
            action: "setRemoteGasData".to_string(),
            message_type: SET_REMOTE_GAS_DATA.to_string(),
            target: destination_config.gas_oracle.clone(),
            selector: Some("0xf3a1495f".to_string()),
            calldata: Some(calldata),
            command: None,
            signer: Some(TxSigner {
                signer_profile: target.config.write.signer_profile.clone(),
                address: Some(owner.clone()),
            }),
            message: serde_json::json!({
                "igp": destination_config.igp_address,
                "gasOracle": destination_config.gas_oracle,
                "gasOracleOwner": owner,
                "remoteGasData": {
                    "remoteDomain": target.remote.domain_id,
                    "tokenExchangeRate": proposed.token_exchange_rate.as_str(),
                    "gasPrice": proposed.gas_price.as_str()
                },
                "preservedGasOverhead": destination_config.gas_overhead
            }),
            notes: vec![
                "review artifact only; transaction submission is not implemented".to_string(),
                "calldata updates StorageGasOracle remote gas data only; IGP gasOracle and gasOverhead are preserved".to_string(),
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
            "EVM verification".to_string(),
        ))
    }
}

fn parse_u128_config(label: &str, value: &str) -> Result<u128> {
    value.parse::<u128>().map_err(|source| {
        IgpOracleError::Policy(format!(
            "proposed EVM {label} value {value} does not fit uint128: {source}"
        ))
    })
}
