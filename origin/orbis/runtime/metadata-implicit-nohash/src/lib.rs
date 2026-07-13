//! Isolated native runtime compiled without `RUNTIME_METADATA_HASH`.

use frame_support::derive_impl;
use sp_runtime::traits::{Hash, TransactionExtension};

#[cfg(test)]

pub type Block = frame_system::mocking::MockBlock<NoHashRuntime>;

frame_support::construct_runtime! {
	pub enum NoHashRuntime {
		System: frame_system,
	}
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for NoHashRuntime {
	type Block = Block;
}

#[derive(Debug, Eq, PartialEq)]
pub struct NoHashArtifact {
	pub error: sp_runtime::transaction_validity::TransactionValidityError,
	pub weight: frame_support::weights::Weight,
	pub state: [u8; 32],
}

pub fn exercise() -> NoHashArtifact {
	use codec::{DecodeAll, Encode};
	let encoded =
		frame_metadata_hash_extension::CheckMetadataHash::<NoHashRuntime>::new(true).encode();
	let decoded = frame_metadata_hash_extension::CheckMetadataHash::<NoHashRuntime>::decode_all(
		&mut encoded.as_slice(),
	)
	.expect("enabled metadata extension round-trips exactly");
	let call = RuntimeCall::System(frame_system::Call::remark { remark: encoded.clone() });
	NoHashArtifact {
		error: bulletin_pallets_common::resolve_metadata_implicit::<RuntimeCall, _>(&decoded)
			.expect_err("no-hash build must reject enabled mode"),
		weight: decoded.weight(&call),
		state: sp_runtime::traits::BlakeTwo256::hash(&encoded).into(),
	}
}

#[cfg(test)]
mod tests {
	#[test]
	fn enabled_metadata_without_compiled_hash_is_exact_cannot_lookup() {
		let artifact = super::exercise();
		assert_eq!(
			artifact.error,
			sp_runtime::transaction_validity::UnknownTransaction::CannotLookup.into(),
		);
		assert_eq!(artifact.weight, frame_support::weights::Weight::zero());
		assert_ne!(artifact.state, [0; 32]);
	}
}
