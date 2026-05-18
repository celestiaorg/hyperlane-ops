use std::time::Duration;

use alloy::{
    network::{EthereumWallet, TransactionBuilder},
    primitives::{Address, Bytes},
    providers::{Provider, ProviderBuilder},
    rpc::types::TransactionRequest,
    signers::local::PrivateKeySigner,
};

use crate::error::{IgpOracleError, Result};

const RECEIPT_TIMEOUT_SECS: u64 = 120;

pub struct SubmittedTx {
    pub tx_hash: String,
    pub block_number: Option<u64>,
    pub signer_address: String,
}

pub async fn submit_calldata(
    rpc_url: &str,
    private_key_hex: &str,
    to: &str,
    calldata: &str,
) -> Result<SubmittedTx> {
    let signer: PrivateKeySigner = private_key_hex
        .parse()
        .map_err(|err| IgpOracleError::InvalidConfig(format!("invalid EVM private key: {err}")))?;
    let signer_address = signer.address().to_string();

    let to_address: Address = to.parse().map_err(|err| {
        IgpOracleError::InvalidConfig(format!("invalid EVM target address {to}: {err}"))
    })?;
    let data: Bytes = calldata.parse().map_err(|err| {
        IgpOracleError::Policy(format!("invalid EVM calldata hex {calldata}: {err}"))
    })?;
    let rpc = rpc_url.parse().map_err(|err| {
        IgpOracleError::OnchainRead(format!("invalid EVM RPC URL {rpc_url}: {err}"))
    })?;

    let wallet = EthereumWallet::from(signer);
    let provider = ProviderBuilder::new().wallet(wallet).connect_http(rpc);

    let tx = TransactionRequest::default()
        .with_to(to_address)
        .with_input(data);

    let pending = provider
        .send_transaction(tx)
        .await
        .map_err(|err| IgpOracleError::OnchainRead(format!("EVM tx broadcast failed: {err}")))?
        .with_timeout(Some(Duration::from_secs(RECEIPT_TIMEOUT_SECS)));

    let tx_hash = pending.tx_hash().to_string();
    let receipt = pending.get_receipt().await.map_err(|err| {
        IgpOracleError::OnchainRead(format!(
            "EVM tx {tx_hash} did not produce a receipt within {RECEIPT_TIMEOUT_SECS}s: {err}"
        ))
    })?;

    if !receipt.status() {
        return Err(IgpOracleError::OnchainRead(format!(
            "EVM tx {tx_hash} reverted on chain"
        )));
    }

    Ok(SubmittedTx {
        tx_hash: receipt.transaction_hash.to_string(),
        block_number: receipt.block_number,
        signer_address,
    })
}

pub fn signer_address_from_key(private_key_hex: &str) -> Result<String> {
    let signer: PrivateKeySigner = private_key_hex
        .parse()
        .map_err(|err| IgpOracleError::InvalidConfig(format!("invalid EVM private key: {err}")))?;
    Ok(signer.address().to_string())
}

pub fn same_evm_address(a: &str, b: &str) -> bool {
    let norm = |value: &str| value.strip_prefix("0x").unwrap_or(value).to_ascii_lowercase();
    norm(a) == norm(b)
}
