use codec::{Decode, Encode};

use crate::*;

/// An on-chain schema written by an issuer.
#[derive(Clone, Debug, Encode, Decode, PartialEq)]
pub struct RegistrarDetails<T: Config> {
	pub block_number: BlockNumberOf<T>,
	/// The flag indicating the status of the entity account.
	pub revoked: bool,
}
