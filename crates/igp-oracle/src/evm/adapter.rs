use async_trait::async_trait;

use alloy::primitives::Address;

use crate::{
    adapter::{ChainAdapter, SignerAuthStatus},
    config::{SignerConfig, WriteMethod},
    error::{IgpOracleError, Result},
    evm::{
        query::{
            encode_set_remote_gas_data, read_destination_gas_config, read_igp_config, read_owner,
            SET_REMOTE_GAS_DATA,
        },
        submit::{same_evm_address, signer_address_from_key, submit_calldata},
    },
    models::{
        ChainMetadata, ChainProtocol, ConfiguredRemoteDomain, CoreAddresses, IgpConfig,
        IgpConfigRead, ReconciliationTarget, TxPayload, TxPlan, TxReceipt, TxSigner,
        VerificationResult,
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
        read_igp_config(target).await
    }

    async fn plan_update(
        &self,
        target: &ReconciliationTarget,
        proposed: &IgpConfig,
    ) -> Result<TxPlan> {
        let destination_config = read_destination_gas_config(target).await?;

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
        let owner = read_owner(
            &destination_config.endpoint,
            &destination_config.gas_oracle,
        )
        .await?;

        Ok(TxPlan {
            protocol: ChainProtocol::Ethereum,
            action: "setRemoteGasData".to_string(),
            message_type: SET_REMOTE_GAS_DATA.to_string(),
            target: destination_config.gas_oracle.clone(),
            selector: Some("0xf3a1495f".to_string()),
            calldata: Some(calldata),
            signer: Some(TxSigner {
                signer_profile: target.config.write.signer_profile.clone(),
                address: Some(owner),
            }),
            payload: TxPayload::EvmSetRemoteGasData {
                gas_oracle: destination_config.gas_oracle.clone(),
                remote_domain: target.remote.domain_id,
                token_exchange_rate,
                gas_price,
            },
        })
    }

    async fn submit_update(
        &self,
        target: &ReconciliationTarget,
        plan: &TxPlan,
        signer: &SignerConfig,
    ) -> Result<TxReceipt> {
        if target.config.write.method != WriteMethod::Evm {
            return Err(IgpOracleError::InvalidConfig(
                "EVM write method must be evm".to_string(),
            ));
        }

        let TxPayload::EvmSetRemoteGasData { gas_oracle, .. } = &plan.payload else {
            return Err(IgpOracleError::UnsupportedWrite);
        };
        let calldata = plan.calldata.as_deref().ok_or_else(|| {
            IgpOracleError::InvalidTarget(
                "EVM submit requires calldata on the tx plan".to_string(),
            )
        })?;
        let rpc_url = target
            .origin
            .rpc_urls
            .first()
            .ok_or_else(|| {
                IgpOracleError::OnchainRead(format!(
                    "origin chain {} has no rpcUrls entry",
                    target.origin.name
                ))
            })?
            .http
            .as_str();

        let private_key = signer.load_private_key_hex()?;
        let signer_address = signer_address_from_key(&private_key)?;
        let on_chain_owner = read_owner(rpc_url, gas_oracle).await?;
        if !same_evm_address(&signer_address, &on_chain_owner) {
            return Err(IgpOracleError::InvalidTarget(format!(
                "EVM signer {signer_address} is not the StorageGasOracle owner {on_chain_owner} for {} -> {}",
                target.origin.name, target.remote.name
            )));
        }

        let submitted = submit_calldata(rpc_url, &private_key, gas_oracle, calldata).await?;
        Ok(TxReceipt {
            tx_hash: submitted.tx_hash,
            height: submitted.block_number,
        })
    }

    fn check_signer_authorization(
        &self,
        tx_signer: &TxSigner,
        signer_config: &SignerConfig,
    ) -> Result<SignerAuthStatus> {
        let authorized = tx_signer.address.as_deref().ok_or_else(|| {
            IgpOracleError::InvalidTarget(
                "EVM write target has no authorized signer address".to_string(),
            )
        })?;
        let configured: Address = signer_config.from.parse().map_err(|err| {
            IgpOracleError::InvalidConfig(format!(
                "configured signer {} is not a valid EVM address: {err}",
                signer_config.from
            ))
        })?;
        let authorized_address: Address = authorized.parse().map_err(|err| {
            IgpOracleError::InvalidTarget(format!(
                "authorized signer {authorized} is not a valid EVM address: {err}"
            ))
        })?;
        if configured != authorized_address {
            return Err(IgpOracleError::InvalidTarget(format!(
                "configured signer {configured} is not authorized for EVM target; expected {authorized_address}"
            )));
        }
        Ok(SignerAuthStatus::AddressMatch)
    }

    async fn verify_update(
        &self,
        target: &ReconciliationTarget,
        expected: &IgpConfig,
    ) -> Result<VerificationResult> {
        let actual = read_igp_config(target).await?.config;
        if actual.gas_price == expected.gas_price
            && actual.token_exchange_rate == expected.token_exchange_rate
        {
            Ok(VerificationResult {
                success: true,
                reason: None,
            })
        } else {
            Ok(VerificationResult {
                success: false,
                reason: Some(format!(
                    "remote gas data mismatch after submit: on-chain ({}, {}) vs expected ({}, {})",
                    actual.gas_price,
                    actual.token_exchange_rate,
                    expected.gas_price,
                    expected.token_exchange_rate
                )),
            })
        }
    }
}

fn parse_u128_config(label: &str, value: &str) -> Result<u128> {
    value.parse::<u128>().map_err(|source| {
        IgpOracleError::Policy(format!(
            "proposed EVM {label} value {value} does not fit uint128: {source}"
        ))
    })
}
