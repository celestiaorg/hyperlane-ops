use tonic::codegen::*;

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GasOracle {
    #[prost(string, tag = "1")]
    pub token_exchange_rate: String,
    #[prost(string, tag = "2")]
    pub gas_price: String,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DestinationGasConfig {
    #[prost(uint32, tag = "1")]
    pub remote_domain: u32,
    #[prost(message, optional, tag = "2")]
    pub gas_oracle: Option<GasOracle>,
    #[prost(string, tag = "3")]
    pub gas_overhead: String,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct InterchainGasPaymaster {
    #[prost(string, tag = "1")]
    pub id: String,
    #[prost(string, tag = "2")]
    pub owner: String,
    #[prost(string, tag = "3")]
    pub denom: String,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct QueryIgpRequest {
    #[prost(string, tag = "1")]
    pub id: String,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct QueryIgpResponse {
    #[prost(message, optional, tag = "1")]
    pub igp: Option<InterchainGasPaymaster>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct QueryDestinationGasConfigsRequest {
    #[prost(string, tag = "1")]
    pub id: String,
    #[prost(message, optional, tag = "2")]
    pub pagination: Option<crate::proto::cosmos::base::query::v1beta1::PageRequest>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct QueryDestinationGasConfigsResponse {
    #[prost(message, repeated, tag = "1")]
    pub destination_gas_configs: Vec<DestinationGasConfig>,
    #[prost(message, optional, tag = "2")]
    pub pagination: Option<crate::proto::cosmos::base::query::v1beta1::PageResponse>,
}

pub mod query_client {
    use super::*;

    #[derive(Debug, Clone)]
    pub struct QueryClient<T> {
        inner: tonic::client::Grpc<T>,
    }

    impl QueryClient<tonic::transport::Channel> {
        pub async fn connect<D>(dst: D) -> std::result::Result<Self, tonic::transport::Error>
        where
            D: std::convert::TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn))
        }
    }

    impl<T> QueryClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody>,
        T::Error: Into<StdError>,
        T::ResponseBody: Body<Data = Bytes> + Send + 'static,
        <T::ResponseBody as Body>::Error: Into<StdError> + Send,
    {
        pub fn new(inner: T) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self { inner }
        }

        pub async fn igp(
            &mut self,
            request: impl tonic::IntoRequest<QueryIgpRequest>,
        ) -> std::result::Result<tonic::Response<QueryIgpResponse>, tonic::Status> {
            self.inner.ready().await.map_err(|err| {
                tonic::Status::unknown(format!("service was not ready: {}", err.into()))
            })?;

            let codec = tonic::codec::ProstCodec::default();
            let path =
                http::uri::PathAndQuery::from_static("/hyperlane.core.post_dispatch.v1.Query/Igp");
            let mut request = request.into_request();
            request.extensions_mut().insert(tonic::GrpcMethod::new(
                "hyperlane.core.post_dispatch.v1.Query",
                "Igp",
            ));
            self.inner.unary(request, path, codec).await
        }

        pub async fn destination_gas_configs(
            &mut self,
            request: impl tonic::IntoRequest<QueryDestinationGasConfigsRequest>,
        ) -> std::result::Result<tonic::Response<QueryDestinationGasConfigsResponse>, tonic::Status>
        {
            self.inner.ready().await.map_err(|err| {
                tonic::Status::unknown(format!("service was not ready: {}", err.into()))
            })?;

            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/hyperlane.core.post_dispatch.v1.Query/DestinationGasConfigs",
            );
            let mut request = request.into_request();
            request.extensions_mut().insert(tonic::GrpcMethod::new(
                "hyperlane.core.post_dispatch.v1.Query",
                "DestinationGasConfigs",
            ));
            self.inner.unary(request, path, codec).await
        }
    }
}
