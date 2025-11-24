use codec::{Decode, Encode};
use scale_info::TypeInfo;

/// Generic attribute update for token pallet.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub struct TokenAttributeInput {
	pub key: Vec<u8>,
	pub value: Vec<u8>,
}
