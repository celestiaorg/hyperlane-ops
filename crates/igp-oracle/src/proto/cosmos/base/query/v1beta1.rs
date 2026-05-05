#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PageRequest {
    #[prost(bytes = "vec", tag = "1")]
    pub key: Vec<u8>,
    #[prost(uint64, tag = "2")]
    pub offset: u64,
    #[prost(uint64, tag = "3")]
    pub limit: u64,
    #[prost(bool, tag = "4")]
    pub count_total: bool,
    #[prost(bool, tag = "5")]
    pub reverse: bool,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PageResponse {
    #[prost(bytes = "vec", tag = "1")]
    pub next_key: Vec<u8>,
    #[prost(uint64, tag = "2")]
    pub total: u64,
}
