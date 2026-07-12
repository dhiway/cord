//! Isolated native runtime compiled without `RUNTIME_METADATA_HASH`.

use frame_support::derive_impl;

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

pub fn resolve(
) -> Result<Option<[u8; 32]>, sp_runtime::transaction_validity::TransactionValidityError> {
	let extension = frame_metadata_hash_extension::CheckMetadataHash::<NoHashRuntime>::new(true);
	bulletin_pallets_common::resolve_metadata_implicit::<RuntimeCall, _>(&extension)
}

#[cfg(test)]
mod tests {
	#[test]
	fn enabled_metadata_without_compiled_hash_is_exact_cannot_lookup() {
		assert_eq!(
			super::resolve(),
			Err(sp_runtime::transaction_validity::UnknownTransaction::CannotLookup.into()),
		);
	}
}
