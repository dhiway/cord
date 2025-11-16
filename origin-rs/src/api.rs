#![allow(clippy::all)]
#![allow(dead_code)]

#[subxt::subxt(runtime_metadata_path = "metadata/origin.scale")]
pub mod runtime_origin {}

#[subxt::subxt(runtime_metadata_path = "metadata/origin-hub.scale")]
pub mod runtime_hub {}
