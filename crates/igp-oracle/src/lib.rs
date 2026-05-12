pub mod adapter;
pub mod artifacts;
pub mod cli;
pub mod config;
pub mod cosmosnative;
pub mod data;
pub mod error;
pub mod evm;
pub mod models;
pub mod plan;
pub mod policy;
pub mod proto;
pub mod reconcile;
pub mod registry;
pub mod resolver;

pub use cli::{Cli, Command, ReconcileArgs};
pub use error::{IgpOracleError, Result};
pub use reconcile::run_reconcile;

use adapter::ChainAdapter;
use cosmosnative::adapter::CosmosNativeAdapter;
use evm::adapter::EvmAdapter;
use models::ChainProtocol;

pub fn adapter_for(protocol: ChainProtocol) -> Box<dyn ChainAdapter> {
    match protocol {
        ChainProtocol::CosmosNative => Box::<CosmosNativeAdapter>::default(),
        ChainProtocol::Ethereum => Box::<EvmAdapter>::default(),
    }
}

#[cfg(test)]
mod tests {
    use crate::{adapter_for, models::ChainProtocol};

    #[test]
    fn adapter_factory_returns_cosmosnative_adapter() {
        let adapter = adapter_for(ChainProtocol::CosmosNative);
        assert_eq!(adapter.protocol(), ChainProtocol::CosmosNative);
    }

    #[test]
    fn adapter_factory_returns_evm_adapter() {
        let adapter = adapter_for(ChainProtocol::Ethereum);
        assert_eq!(adapter.protocol(), ChainProtocol::Ethereum);
    }
}
