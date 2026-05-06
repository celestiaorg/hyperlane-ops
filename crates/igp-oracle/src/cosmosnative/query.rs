use tonic::{transport::Endpoint, Request};

use crate::{
    error::{IgpOracleError, Result},
    models::{ConfiguredRemoteDomain, CurrentIgpConfig, OnChainReadSource},
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
        let configs = self.destination_gas_configs(chain_name, igp_id).await?;
        let config = configs
            .into_iter()
            .find(|config| config.remote_domain == remote_domain)
            .ok_or_else(|| {
                IgpOracleError::DataSource(format!(
                    "destination gas config for remote domain {remote_domain} was not found"
                ))
            })?;

        Ok((config.current, config.source))
    }

    pub async fn destination_gas_configs(
        &self,
        chain_name: &str,
        igp_id: &str,
    ) -> Result<Vec<ConfiguredRemoteDomain>> {
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

        let source = OnChainReadSource {
            protocol: "cosmosnative".to_string(),
            endpoint: Some(self.endpoint.clone()),
            query: DESTINATION_GAS_CONFIGS_QUERY.to_string(),
        };

        parse_destination_gas_configs(response.destination_gas_configs, source)
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
        Endpoint::new(self.endpoint.clone())
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

fn parse_destination_gas_configs(
    configs: Vec<DestinationGasConfig>,
    source: OnChainReadSource,
) -> Result<Vec<ConfiguredRemoteDomain>> {
    configs
        .into_iter()
        .map(|config| {
            let remote_domain = config.remote_domain;
            let current = parse_single_destination_gas_config(config)?;
            Ok(ConfiguredRemoteDomain {
                remote_domain,
                current,
                source: source.clone(),
            })
        })
        .collect()
}

fn parse_single_destination_gas_config(config: DestinationGasConfig) -> Result<CurrentIgpConfig> {
    let remote_domain = config.remote_domain;
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

        let current = parse_single_destination_gas_config(
            configs.into_iter().next().expect("fixture config"),
        )
        .expect("destination config");

        assert_eq!(current.gas_price, "1000000000");
        assert_eq!(current.token_exchange_rate, "1");
        assert_eq!(current.gas_overhead, 300000);
    }

    #[test]
    fn parses_multiple_celestia_destination_gas_configs() {
        let configs = vec![
            DestinationGasConfig {
                remote_domain: 1,
                gas_oracle: Some(GasOracle {
                    token_exchange_rate: "101".to_string(),
                    gas_price: "300000000".to_string(),
                }),
                gas_overhead: "174289".to_string(),
            },
            DestinationGasConfig {
                remote_domain: 2147483647,
                gas_oracle: Some(GasOracle {
                    token_exchange_rate: "1".to_string(),
                    gas_price: "1000000000".to_string(),
                }),
                gas_overhead: "300000".to_string(),
            },
        ];
        let source = OnChainReadSource {
            protocol: "cosmosnative".to_string(),
            endpoint: Some("test://grpc".to_string()),
            query: DESTINATION_GAS_CONFIGS_QUERY.to_string(),
        };

        let parsed = parse_destination_gas_configs(configs, source).expect("configs should parse");

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].remote_domain, 1);
        assert_eq!(parsed[0].current.token_exchange_rate, "101");
        assert_eq!(parsed[1].remote_domain, 2147483647);
        assert_eq!(parsed[1].current.gas_overhead, 300000);
    }
}
