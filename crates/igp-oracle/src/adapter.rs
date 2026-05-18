use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::Serialize;

use crate::{
    config::SignerConfig,
    error::Result,
    models::{
        ChainMetadata, ChainProtocol, ConfiguredRemoteDomain, CoreAddresses, GasPriceSample,
        IgpConfig, IgpConfigRead, ReconciliationTarget, TxPlan, TxReceipt, TxSigner,
        VerificationResult,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SignerAuthStatus {
    AddressMatch,
    KeyAliasUnverified,
    AuthorityUnavailable,
}

#[async_trait]
pub trait ChainAdapter: Send + Sync {
    fn protocol(&self) -> ChainProtocol;

    async fn list_igp_destination_configs(
        &self,
        origin: &ChainMetadata,
        origin_addresses: &CoreAddresses,
    ) -> Result<Vec<ConfiguredRemoteDomain>>;

    async fn read_igp_config(&self, target: &ReconciliationTarget) -> Result<IgpConfigRead>;

    async fn plan_update(
        &self,
        target: &ReconciliationTarget,
        proposed: &IgpConfig,
    ) -> Result<TxPlan>;

    async fn submit_update(
        &self,
        target: &ReconciliationTarget,
        plan: &TxPlan,
        signer: &SignerConfig,
    ) -> Result<TxReceipt>;

    async fn verify_update(
        &self,
        target: &ReconciliationTarget,
        expected: &IgpConfig,
    ) -> Result<VerificationResult>;

    fn check_signer_authorization(
        &self,
        tx_signer: &TxSigner,
        signer_config: &SignerConfig,
    ) -> Result<SignerAuthStatus>;
}

#[async_trait]
pub trait GasAdapter: Send + Sync {
    async fn remote_gas_price(&self, target: &ReconciliationTarget) -> Result<GasPriceSample>;
}

#[async_trait]
pub trait PriceAdapter: Send + Sync {
    async fn native_token_price_usd(&self, chain_name: &str) -> Result<Decimal>;
}

pub type ChainAdapterFactory = dyn Fn(ChainProtocol) -> Box<dyn ChainAdapter> + Sync;
