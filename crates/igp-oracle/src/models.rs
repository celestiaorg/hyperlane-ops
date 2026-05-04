use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::config::TargetConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChainProtocol {
    Ethereum,
    CosmosNative,
}

impl ChainProtocol {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ethereum => "ethereum",
            Self::CosmosNative => "cosmosnative",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChainId {
    String(String),
    Number(u64),
}

impl ChainId {
    pub fn as_string(&self) -> String {
        match self {
            Self::String(value) => value.clone(),
            Self::Number(value) => value.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UrlEntry {
    pub http: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeToken {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub denom: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataGasPrice {
    pub amount: String,
    pub denom: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainMetadata {
    pub name: String,
    pub domain_id: u32,
    pub chain_id: ChainId,
    pub protocol: ChainProtocol,
    pub native_token: NativeToken,
    #[serde(default)]
    pub rpc_urls: Vec<UrlEntry>,
    #[serde(default)]
    pub grpc_urls: Vec<UrlEntry>,
    #[serde(default)]
    pub rest_urls: Vec<UrlEntry>,
    pub gas_price: Option<MetadataGasPrice>,
    pub bech32_prefix: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreAddresses {
    pub interchain_gas_paymaster: Option<String>,
    pub mailbox: Option<String>,
    pub interchain_security_module: Option<String>,
    pub merkle_tree_hook: Option<String>,
    #[serde(flatten)]
    pub other: BTreeMap<String, serde_yaml::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReconciliationTarget {
    pub origin: ChainMetadata,
    pub remote: ChainMetadata,
    pub origin_addresses: CoreAddresses,
    pub config: TargetConfig,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentIgpConfig {
    pub gas_price: String,
    pub token_exchange_rate: String,
    pub gas_overhead: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposedIgpConfig {
    pub gas_price: String,
    pub token_exchange_rate: String,
    pub gas_overhead: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TxPlan {
    pub target: String,
    pub selector: Option<String>,
    pub calldata: Option<String>,
    pub command: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TxReceipt {
    pub tx_hash: String,
    pub height: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerificationResult {
    pub success: bool,
    pub reason: Option<String>,
}
