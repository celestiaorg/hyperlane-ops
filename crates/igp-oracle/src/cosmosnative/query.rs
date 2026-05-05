use tonic::{transport::Endpoint, Request};

use crate::{
    error::{IgpOracleError, Result},
    models::{CurrentIgpConfig, OnChainReadSource},
    proto::hyperlane::core::post_dispatch::v1::{
        query_client::QueryClient, DestinationGasConfig, QueryDestinationGasConfigsRequest,
        QueryIgpRequest,
    },
};

const DESTINATION_GAS_CONFIGS_QUERY: &str =
    "/hyperlane.core.post_dispatch.v1.Query/DestinationGasConfigs";

#[derive(Debug, Clone)]
pub struct CosmosNativeQueryClient {
    endpoint: String,
}

impl CosmosNativeQueryClient {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
        }
    }

    pub async fn destination_gas_config(
        &self,
        chain_name: &str,
        igp_id: &str,
        remote_domain: u32,
    ) -> Result<(CurrentIgpConfig, OnChainReadSource)> {
        let channel = self.connect(chain_name).await?;
        let mut client = QueryClient::new(channel);
        let response = client
            .destination_gas_configs(Request::new(QueryDestinationGasConfigsRequest {
                id: igp_id.to_string(),
                pagination: None,
            }))
            .await
            .map_err(|source| {
                IgpOracleError::DataSource(format!(
                    "destination gas config gRPC query failed for {chain_name} IGP {igp_id}: {source}"
                ))
            })?
            .into_inner();

        let config = parse_destination_gas_config(response.destination_gas_configs, remote_domain)?;
        let source = OnChainReadSource {
            protocol: "cosmosnative".to_string(),
            endpoint: Some(self.endpoint.clone()),
            query: DESTINATION_GAS_CONFIGS_QUERY.to_string(),
        };

        Ok((config, source))
    }

    pub async fn igp_owner(&self, chain_name: &str, igp_id: &str) -> Result<String> {
        let channel = self.connect(chain_name).await?;
        let mut client = QueryClient::new(channel);
        let response = client
            .igp(Request::new(QueryIgpRequest {
                id: igp_id.to_string(),
            }))
            .await
            .map_err(|source| {
                IgpOracleError::DataSource(format!(
                    "IGP gRPC query failed for {chain_name} IGP {igp_id}: {source}"
                ))
            })?
            .into_inner();

        response
            .igp
            .map(|igp| igp.owner)
            .filter(|owner| !owner.is_empty())
            .ok_or_else(|| {
                IgpOracleError::DataSource(format!(
                    "IGP gRPC query for {chain_name} IGP {igp_id} returned no owner"
                ))
            })
    }

    async fn connect(&self, chain_name: &str) -> Result<tonic::transport::Channel> {
        Endpoint::from_shared(self.endpoint.clone())
            .map_err(|source| {
                IgpOracleError::DataSource(format!(
                    "invalid gRPC endpoint for {chain_name}: {source}"
                ))
            })?
            .connect_timeout(std::time::Duration::from_secs(15))
            .tcp_nodelay(true)
            .connect()
            .await
            .map_err(|source| {
                IgpOracleError::DataSource(format!(
                    "failed to connect to {chain_name} gRPC endpoint {}: {source}",
                    self.endpoint
                ))
            })
    }
}

fn parse_destination_gas_config(
    configs: Vec<DestinationGasConfig>,
    remote_domain: u32,
) -> Result<CurrentIgpConfig> {
    let config = configs
        .into_iter()
        .find(|config| config.remote_domain == remote_domain)
        .ok_or_else(|| {
            IgpOracleError::DataSource(format!(
                "destination gas config for remote domain {remote_domain} was not found"
            ))
        })?;
    let gas_oracle = config.gas_oracle.ok_or_else(|| {
        IgpOracleError::DataSource(format!(
            "destination gas config for remote domain {remote_domain} has no gas oracle"
        ))
    })?;

    let gas_overhead = config.gas_overhead.parse::<u64>().map_err(|source| {
        IgpOracleError::DataSource(format!(
            "invalid gasOverhead {} for remote domain {}: {source}",
            config.gas_overhead, remote_domain
        ))
    })?;

    Ok(CurrentIgpConfig {
        gas_price: gas_oracle.gas_price,
        token_exchange_rate: gas_oracle.token_exchange_rate,
        gas_overhead,
    })
}

#[cfg(test)]
mod tests {
    use crate::proto::hyperlane::core::post_dispatch::v1::{DestinationGasConfig, GasOracle};

    use super::*;

    #[test]
    fn parses_celestia_destination_gas_config() {
        let configs = vec![DestinationGasConfig {
            remote_domain: 2147483647,
            gas_oracle: Some(GasOracle {
                token_exchange_rate: "1".to_string(),
                gas_price: "1000000000".to_string(),
            }),
            gas_overhead: "300000".to_string(),
        }];

        let current =
            parse_destination_gas_config(configs, 2147483647).expect("destination config");

        assert_eq!(current.gas_price, "1000000000");
        assert_eq!(current.token_exchange_rate, "1");
        assert_eq!(current.gas_overhead, 300000);
    }
}
