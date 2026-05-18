//! Hand-maintained protobuf bindings used by `igp-oracle`.
//!
//! These mirror the Hyperlane Cosmos gRPC query shapes needed by the dry-run
//! reader. The upstream service path currently targeted is:
//! `/hyperlane.core.post_dispatch.v1.Query/DestinationGasConfigs`.

pub mod cosmos;
pub mod hyperlane;
