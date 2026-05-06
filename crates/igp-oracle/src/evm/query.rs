use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::{
    data::http_client,
    error::{IgpOracleError, Result},
    models::{CurrentIgpConfig, IgpConfigRead, OnChainReadSource, ReconciliationTarget},
};

const DESTINATION_GAS_CONFIGS: &str = "destinationGasConfigs(uint32)";
const DESTINATION_GAS_LIMIT: &str = "destinationGasLimit(uint32,uint256)";
const REMOTE_GAS_DATA: &str = "remoteGasData(uint32)";
const DESTINATION_GAS_CONFIGS_SELECTOR: [u8; 4] = [0x43, 0xc4, 0x67, 0xc0];
const DESTINATION_GAS_LIMIT_SELECTOR: [u8; 4] = [0x26, 0xd5, 0xb1, 0xa6];
const REMOTE_GAS_DATA_SELECTOR: [u8; 4] = [0xb0, 0x8e, 0x56, 0xd0];
const EVM_IGP_QUERY: &str =
    "eth_call: destinationGasConfigs(uint32), destinationGasLimit(uint32,uint256), remoteGasData(uint32)";

#[derive(Debug, Clone)]
pub struct EvmIgpReader {
    client: Client,
}

impl EvmIgpReader {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: http_client()?,
        })
    }

    pub async fn read_igp_config(&self, target: &ReconciliationTarget) -> Result<IgpConfigRead> {
        let igp_address = target
            .origin_addresses
            .interchain_gas_paymaster
            .as_deref()
            .ok_or_else(|| {
                IgpOracleError::Registry(format!(
                    "origin chain {} has no interchainGasPaymaster address",
                    target.origin.name
                ))
            })?;
        let igp_address = normalize_evm_address(igp_address, "interchainGasPaymaster")?;
        let endpoint = target
            .origin
            .rpc_urls
            .first()
            .ok_or_else(|| {
                IgpOracleError::DataSource(format!(
                    "origin chain {} has no rpcUrls entry",
                    target.origin.name
                ))
            })?
            .http
            .as_str();

        let remote_domain = target.remote.domain_id;
        let gas_oracle = self
            .read_destination_gas_oracle(endpoint, &igp_address, remote_domain)
            .await?;
        if is_zero_address(&gas_oracle) {
            return Err(IgpOracleError::DataSource(format!(
                "IGP {igp_address} on {} has no gas oracle configured for remote domain {remote_domain}",
                target.origin.name
            )));
        }

        let gas_overhead = self
            .read_destination_gas_overhead(endpoint, &igp_address, remote_domain)
            .await?;
        let remote_gas_data = self
            .read_remote_gas_data(endpoint, &gas_oracle, remote_domain)
            .await?;

        Ok(IgpConfigRead {
            config: CurrentIgpConfig {
                gas_price: remote_gas_data.gas_price.to_string(),
                token_exchange_rate: remote_gas_data.token_exchange_rate.to_string(),
                gas_overhead,
            },
            source: OnChainReadSource {
                protocol: "ethereum".to_string(),
                endpoint: Some(endpoint.to_string()),
                query: EVM_IGP_QUERY.to_string(),
            },
        })
    }

    async fn read_destination_gas_oracle(
        &self,
        endpoint: &str,
        igp_address: &str,
        remote_domain: u32,
    ) -> Result<String> {
        let data = encode_u32_call(DESTINATION_GAS_CONFIGS, remote_domain);
        let output = self.eth_call(endpoint, igp_address, &data).await?;
        decode_address_word(&output, 0, "destinationGasConfigs.gasOracle")
    }

    async fn read_destination_gas_overhead(
        &self,
        endpoint: &str,
        igp_address: &str,
        remote_domain: u32,
    ) -> Result<u64> {
        let data = encode_u32_u256_call(DESTINATION_GAS_LIMIT, remote_domain, 0);
        let output = self.eth_call(endpoint, igp_address, &data).await?;
        let gas_limit = decode_u128_word(&output, 0, "destinationGasLimit")?;
        u64::try_from(gas_limit).map_err(|source| {
            IgpOracleError::DataSource(format!(
                "destinationGasLimit for remote domain {remote_domain} does not fit in u64: {source}"
            ))
        })
    }

    async fn read_remote_gas_data(
        &self,
        endpoint: &str,
        gas_oracle: &str,
        remote_domain: u32,
    ) -> Result<RemoteGasData> {
        let data = encode_u32_call(REMOTE_GAS_DATA, remote_domain);
        let output = self.eth_call(endpoint, gas_oracle, &data).await?;
        Ok(RemoteGasData {
            token_exchange_rate: decode_u128_word(&output, 0, "remoteGasData.tokenExchangeRate")?,
            gas_price: decode_u128_word(&output, 1, "remoteGasData.gasPrice")?,
        })
    }

    async fn eth_call(&self, endpoint: &str, to: &str, data: &str) -> Result<String> {
        let response: JsonRpcResponse = self
            .client
            .post(endpoint)
            .json(&JsonRpcRequest {
                jsonrpc: "2.0",
                method: "eth_call",
                params: vec![
                    serde_json::json!({ "to": to, "data": data }),
                    serde_json::json!("latest"),
                ],
                id: 1,
            })
            .send()
            .await
            .map_err(|source| {
                IgpOracleError::DataSource(format!("eth_call request failed for {to}: {source}"))
            })?
            .error_for_status()
            .map_err(|source| {
                IgpOracleError::DataSource(format!("eth_call HTTP error for {to}: {source}"))
            })?
            .json()
            .await
            .map_err(|source| {
                IgpOracleError::DataSource(format!(
                    "eth_call response parse failed for {to}: {source}"
                ))
            })?;

        if let Some(error) = response.error {
            return Err(IgpOracleError::DataSource(format!(
                "eth_call RPC error for {to}: {}",
                error.message
            )));
        }

        response.result.ok_or_else(|| {
            IgpOracleError::DataSource(format!("eth_call response for {to} had no result"))
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RemoteGasData {
    token_exchange_rate: u128,
    gas_price: u128,
}

#[derive(Debug, Serialize)]
struct JsonRpcRequest {
    jsonrpc: &'static str,
    method: &'static str,
    params: Vec<serde_json::Value>,
    id: u64,
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    result: Option<String>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    message: String,
}

fn normalize_evm_address(value: &str, label: &str) -> Result<String> {
    let raw = value.strip_prefix("0x").unwrap_or(value);
    if raw.len() != 40 || !raw.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(IgpOracleError::Registry(format!(
            "{label} must be a 20-byte EVM address, got {value}"
        )));
    }

    Ok(format!("0x{}", raw.to_ascii_lowercase()))
}

fn is_zero_address(value: &str) -> bool {
    value
        .strip_prefix("0x")
        .is_some_and(|raw| raw.bytes().all(|byte| byte == b'0'))
}

fn encode_u32_call(signature: &str, value: u32) -> String {
    let mut bytes = Vec::with_capacity(4 + 32);
    bytes.extend_from_slice(&selector(signature));
    append_u32_word(&mut bytes, value);
    encode_hex(&bytes)
}

fn encode_u32_u256_call(signature: &str, first: u32, second: u128) -> String {
    let mut bytes = Vec::with_capacity(4 + 64);
    bytes.extend_from_slice(&selector(signature));
    append_u32_word(&mut bytes, first);
    append_u128_word(&mut bytes, second);
    encode_hex(&bytes)
}

fn selector(signature: &str) -> [u8; 4] {
    match signature {
        DESTINATION_GAS_CONFIGS => DESTINATION_GAS_CONFIGS_SELECTOR,
        DESTINATION_GAS_LIMIT => DESTINATION_GAS_LIMIT_SELECTOR,
        REMOTE_GAS_DATA => REMOTE_GAS_DATA_SELECTOR,
        _ => unreachable!("unsupported EVM IGP function signature"),
    }
}

fn append_u32_word(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&[0; 28]);
    bytes.extend_from_slice(&value.to_be_bytes());
}

fn append_u128_word(bytes: &mut Vec<u8>, value: u128) {
    bytes.extend_from_slice(&[0; 16]);
    bytes.extend_from_slice(&value.to_be_bytes());
}

fn decode_address_word(value: &str, word_index: usize, label: &str) -> Result<String> {
    let word = decode_word(value, word_index, label)?;
    if word[..12].iter().any(|byte| *byte != 0) {
        return Err(IgpOracleError::DataSource(format!(
            "{label} was not ABI-encoded as an address"
        )));
    }
    Ok(encode_hex(&word[12..]))
}

fn decode_u128_word(value: &str, word_index: usize, label: &str) -> Result<u128> {
    let word = decode_word(value, word_index, label)?;
    if word[..16].iter().any(|byte| *byte != 0) {
        return Err(IgpOracleError::DataSource(format!(
            "{label} does not fit in u128"
        )));
    }

    let mut raw = [0u8; 16];
    raw.copy_from_slice(&word[16..]);
    Ok(u128::from_be_bytes(raw))
}

fn decode_word(value: &str, word_index: usize, label: &str) -> Result<[u8; 32]> {
    let bytes = decode_hex(value, label)?;
    let start = word_index * 32;
    let end = start + 32;
    if bytes.len() < end {
        return Err(IgpOracleError::DataSource(format!(
            "{label} ABI response was too short: expected at least {end} bytes, got {}",
            bytes.len()
        )));
    }

    let mut word = [0u8; 32];
    word.copy_from_slice(&bytes[start..end]);
    Ok(word)
}

fn decode_hex(value: &str, label: &str) -> Result<Vec<u8>> {
    let raw = value.strip_prefix("0x").unwrap_or(value);
    if !raw.len().is_multiple_of(2) {
        return Err(IgpOracleError::DataSource(format!(
            "{label} hex value has odd length"
        )));
    }

    (0..raw.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&raw[index..index + 2], 16).map_err(|source| {
                IgpOracleError::DataSource(format!("{label} contains invalid hex: {source}"))
            })
        })
        .collect()
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(2 + bytes.len() * 2);
    output.push_str("0x");
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use crate::{
        config::{ClampConfig, GasConfig, TargetConfig, WriteConfig},
        models::{ChainId, ChainMetadata, ChainProtocol, CoreAddresses, NativeToken, UrlEntry},
    };

    use super::*;

    #[test]
    fn computes_known_igp_selectors() {
        assert_eq!(encode_hex(&selector(DESTINATION_GAS_CONFIGS)), "0x43c467c0");
        assert_eq!(encode_hex(&selector(DESTINATION_GAS_LIMIT)), "0x26d5b1a6");
        assert_eq!(encode_hex(&selector(REMOTE_GAS_DATA)), "0xb08e56d0");
    }

    #[test]
    fn encodes_remote_domain_call() {
        assert_eq!(
            encode_u32_call(DESTINATION_GAS_CONFIGS, 2_147_483_647),
            "0x43c467c0000000000000000000000000000000000000000000000000000000007fffffff"
        );
    }

    #[test]
    fn decodes_destination_gas_config_address() {
        let response =
            "0x0000000000000000000000001111111111111111111111111111111111111111000000000000000000000000000000000000000000000000000000000001e240";

        assert_eq!(
            decode_address_word(response, 0, "gasOracle").expect("address should decode"),
            "0x1111111111111111111111111111111111111111"
        );
    }

    #[test]
    fn decodes_remote_gas_data_words() {
        let response =
            "0x00000000000000000000000000000000000000000000000000000002540be4000000000000000000000000000000000000000000000000000000000000000064";

        assert_eq!(
            decode_u128_word(response, 0, "tokenExchangeRate")
                .expect("exchange rate should decode"),
            10_000_000_000
        );
        assert_eq!(
            decode_u128_word(response, 1, "gasPrice").expect("gas price should decode"),
            100
        );
    }

    #[test]
    fn rejects_non_evm_addresses() {
        let err = normalize_evm_address(
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
                method: "evm".to_string(),
                signer_profile: "evm-owner".to_string(),
            },
        }
    }
}
