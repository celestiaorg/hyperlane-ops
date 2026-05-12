use alloy::{
    hex,
    primitives::{Address, U256},
    providers::{Provider, ProviderBuilder},
    sol,
    sol_types::SolCall,
};

use crate::{
    error::{IgpOracleError, Result},
    models::{ChainProtocol, IgpConfig, IgpConfigRead, OnChainReadSource, ReconciliationTarget},
};

pub(crate) const SET_REMOTE_GAS_DATA: &str = "setRemoteGasData((uint32,uint128,uint128))";
const EVM_IGP_QUERY: &str =
    "eth_call: destinationGasConfigs(uint32), destinationGasLimit(uint32,uint256), remoteGasData(uint32)";

sol! {
    #[sol(rpc)]
    interface IInterchainGasPaymaster {
        function destinationGasConfigs(uint32 remoteDomain)
            external view returns (address gasOracle, uint96 gasOverhead);
        function destinationGasLimit(uint32 remoteDomain, uint256 gasLimit)
            external view returns (uint256);
    }
}

sol! {
    #[derive(Debug)]
    struct RemoteGasDataConfig {
        uint32 remoteDomain;
        uint128 tokenExchangeRate;
        uint128 gasPrice;
    }

    #[sol(rpc)]
    interface IStorageGasOracle {
        function owner() external view returns (address);
        function remoteGasData(uint32 remoteDomain)
            external view returns (uint128 tokenExchangeRate, uint128 gasPrice);
        function setRemoteGasData(RemoteGasDataConfig calldata config) external;
    }
}

#[derive(Debug, Clone)]
pub struct EvmIgpReader;

impl EvmIgpReader {
    pub fn new() -> Result<Self> {
        Ok(Self)
    }

    pub async fn read_destination_gas_config(
        &self,
        target: &ReconciliationTarget,
    ) -> Result<EvmDestinationGasConfig> {
        let igp_address_raw = target
            .origin_addresses
            .interchain_gas_paymaster
            .as_deref()
            .ok_or_else(|| {
                IgpOracleError::Registry(format!(
                    "origin chain {} has no interchainGasPaymaster address",
                    target.origin.name
                ))
            })?;
        let igp_address = parse_registry_address(igp_address_raw, "interchainGasPaymaster")?;
        let endpoint = first_rpc(target)?;
        let provider = build_provider(endpoint)?;
        let igp = IInterchainGasPaymaster::new(igp_address, &provider);

        let remote_domain = target.remote.domain_id;

        let gas_oracle = igp
            .destinationGasConfigs(remote_domain)
            .call()
            .await
            .map_err(|err| {
                IgpOracleError::OnchainRead(format!(
                    "destinationGasConfigs eth_call failed for {}: {err}",
                    target.origin.name
                ))
            })?
            .gasOracle;
        if gas_oracle == Address::ZERO {
            return Err(IgpOracleError::OnchainRead(format!(
                "IGP {igp_address} on {} has no gas oracle configured for remote domain {remote_domain}",
                target.origin.name
            )));
        }

        let gas_limit: U256 = igp
            .destinationGasLimit(remote_domain, U256::ZERO)
            .call()
            .await
            .map_err(|err| {
                IgpOracleError::OnchainRead(format!(
                    "destinationGasLimit eth_call failed for {}: {err}",
                    target.origin.name
                ))
            })?;
        let gas_overhead: u64 = gas_limit.try_into().map_err(|err| {
            IgpOracleError::OnchainRead(format!(
                "destinationGasLimit for remote domain {remote_domain} does not fit in u64: {err}"
            ))
        })?;

        Ok(EvmDestinationGasConfig {
            endpoint: endpoint.to_string(),
            igp_address: igp_address.to_string(),
            gas_oracle: gas_oracle.to_string(),
            gas_overhead,
        })
    }

    pub async fn read_igp_config(&self, target: &ReconciliationTarget) -> Result<IgpConfigRead> {
        let destination_config = self.read_destination_gas_config(target).await?;
        let gas_oracle: Address = destination_config.gas_oracle.parse().map_err(|err| {
            IgpOracleError::OnchainRead(format!(
                "gas oracle address {} is not parseable: {err}",
                destination_config.gas_oracle
            ))
        })?;
        let provider = build_provider(&destination_config.endpoint)?;
        let contract = IStorageGasOracle::new(gas_oracle, &provider);
        let remote_data = contract
            .remoteGasData(target.remote.domain_id)
            .call()
            .await
            .map_err(|err| {
                IgpOracleError::OnchainRead(format!(
                    "remoteGasData eth_call failed for {}: {err}",
                    target.origin.name
                ))
            })?;

        Ok(IgpConfigRead {
            config: IgpConfig {
                gas_price: remote_data.gasPrice.to_string(),
                token_exchange_rate: remote_data.tokenExchangeRate.to_string(),
                gas_overhead: destination_config.gas_overhead,
            },
            source: OnChainReadSource {
                protocol: ChainProtocol::Ethereum,
                endpoint: Some(destination_config.endpoint),
                query: EVM_IGP_QUERY.to_string(),
            },
        })
    }

    pub async fn read_owner(&self, endpoint: &str, contract_address: &str) -> Result<String> {
        let address: Address = contract_address.parse().map_err(|err| {
            IgpOracleError::OnchainRead(format!(
                "gas oracle address {contract_address} is not parseable: {err}"
            ))
        })?;
        let provider = build_provider(endpoint)?;
        let contract = IStorageGasOracle::new(address, &provider);
        let owner = contract
            .owner()
            .call()
            .await
            .map_err(|err| IgpOracleError::OnchainRead(format!("owner eth_call failed: {err}")))?;
        Ok(owner.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmDestinationGasConfig {
    pub endpoint: String,
    pub igp_address: String,
    pub gas_oracle: String,
    pub gas_overhead: u64,
}

pub(crate) fn encode_set_remote_gas_data(
    remote_domain: u32,
    token_exchange_rate: u128,
    gas_price: u128,
) -> String {
    let call = IStorageGasOracle::setRemoteGasDataCall {
        config: RemoteGasDataConfig {
            remoteDomain: remote_domain,
            tokenExchangeRate: token_exchange_rate,
            gasPrice: gas_price,
        },
    };
    format!("0x{}", hex::encode(call.abi_encode()))
}

fn build_provider(endpoint: &str) -> Result<impl Provider> {
    let url = endpoint
        .parse()
        .map_err(|err| IgpOracleError::OnchainRead(format!("invalid EVM RPC URL {endpoint}: {err}")))?;
    Ok(ProviderBuilder::new().connect_http(url))
}

fn first_rpc(target: &ReconciliationTarget) -> Result<&str> {
    target
        .origin
        .rpc_urls
        .first()
        .ok_or_else(|| {
            IgpOracleError::OnchainRead(format!(
                "origin chain {} has no rpcUrls entry",
                target.origin.name
            ))
        })
        .map(|entry| entry.http.as_str())
}

fn parse_registry_address(value: &str, label: &str) -> Result<Address> {
    let raw = value.strip_prefix("0x").unwrap_or(value);
    if raw.len() != 40 || !raw.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(IgpOracleError::Registry(format!(
            "{label} must be a 20-byte EVM address, got {value}"
        )));
    }
    value.parse().map_err(|err| {
        IgpOracleError::Registry(format!(
            "{label} {value} is not a valid EVM address: {err}"
        ))
    })
}

#[cfg(test)]
mod tests {
    use crate::{
        config::{ClampConfig, GasConfig, GasMode, TargetConfig, WriteConfig, WriteMethod},
        models::{ChainId, ChainMetadata, ChainProtocol, CoreAddresses, NativeToken, UrlEntry},
    };

    use super::*;

    #[test]
    fn known_selectors_match_solidity_signatures() {
        // SolCall::SELECTOR is computed from the declared sol! signature; this pins
        // the expected on-chain selectors so any accidental signature change is caught.
        assert_eq!(
            hex::encode(IInterchainGasPaymaster::destinationGasConfigsCall::SELECTOR),
            "43c467c0"
        );
        assert_eq!(
            hex::encode(IInterchainGasPaymaster::destinationGasLimitCall::SELECTOR),
            "26d5b1a6"
        );
        assert_eq!(hex::encode(IStorageGasOracle::ownerCall::SELECTOR), "8da5cb5b");
        assert_eq!(
            hex::encode(IStorageGasOracle::remoteGasDataCall::SELECTOR),
            "b08e56d0"
        );
        assert_eq!(
            hex::encode(IStorageGasOracle::setRemoteGasDataCall::SELECTOR),
            "f3a1495f"
        );
    }

    #[test]
    fn encodes_set_remote_gas_data_call() {
        assert_eq!(
            encode_set_remote_gas_data(69_420, 1_607_707_095_149_661_413, 2),
            "0xf3a1495f0000000000000000000000000000000000000000000000000000000000010f2c000000000000000000000000000000000000000000000000164fb915c5454ce50000000000000000000000000000000000000000000000000000000000000002"
        );
    }

    #[test]
    fn rejects_non_evm_addresses() {
        let err = parse_registry_address(
            "0x726f757465725f706f73745f6469737061746368000000040000000000000003",
            "interchainGasPaymaster",
        )
        .expect_err("cosmosnative IGP id is not an EVM address");

        assert!(matches!(err, IgpOracleError::Registry(_)));
    }

    #[tokio::test]
    async fn evm_read_requires_igp_address_before_rpc() {
        let target = ReconciliationTarget {
            origin: test_chain("edentestnet", ChainProtocol::Ethereum, 2_147_483_647),
            remote: test_chain(
                "celestiatestnet",
                ChainProtocol::CosmosNative,
                1_297_040_200,
            ),
            origin_addresses: CoreAddresses::default(),
            config: test_target_config(),
            gas_overhead: 1,
        };

        let err = EvmIgpReader::new()
            .expect("reader should build")
            .read_igp_config(&target)
            .await
            .expect_err("missing IGP address should fail before any RPC call");

        assert!(matches!(err, IgpOracleError::Registry(_)));
    }

    fn test_chain(name: &str, protocol: ChainProtocol, domain_id: u32) -> ChainMetadata {
        ChainMetadata {
            name: name.to_string(),
            domain_id,
            chain_id: ChainId::Number(domain_id as u64),
            protocol,
            native_token: NativeToken {
                name: "Test".to_string(),
                symbol: "TST".to_string(),
                decimals: 18,
                denom: None,
            },
            rpc_urls: vec![UrlEntry {
                http: "http://127.0.0.1:8545".to_string(),
            }],
            grpc_urls: Vec::new(),
            rest_urls: Vec::new(),
            gas_price: None,
            bech32_prefix: None,
        }
    }

    fn test_target_config() -> TargetConfig {
        TargetConfig {
            origin_chain: "edentestnet".to_string(),
            remote_selection: crate::config::RemoteSelection::ConfiguredOnOriginIgp,
            enabled: true,
            gas: GasConfig {
                mode: GasMode::Sample,
                source: "rpc".to_string(),
                min: "1".to_string(),
                max: "1000000000000".to_string(),
            },
            exchange_rate: ClampConfig {
                min: "1".to_string(),
                max: "1000000000000".to_string(),
            },
            write: WriteConfig {
                enabled: true,
                method: WriteMethod::Evm,
                signer_profile: "evm-owner".to_string(),
            },
        }
    }
}
