use crate::types::entity::{BlockRef, EventBlockRecord};
use scale_decode::DecodeAsType;
use serde::{Deserialize, Serialize};

/// Portable representation of `pallet_token::StateEvent` decoded via metadata.
#[derive(Clone, Debug, Serialize, Deserialize, DecodeAsType, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StateEventRecord {
	pub action: Vec<u8>,
	pub digest: [u8; 32],
	pub seal: EventBlockRecord,
}

impl StateEventRecord {
	pub fn block_ref(&self) -> BlockRef {
		BlockRef { height: self.seal.height, index: self.seal.index }
	}
}
