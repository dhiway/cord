// This file is part of CORD – https://cord.network

// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.

//! Provides "fake" runtime API implementations
//!
//! These are used to provide a type that implements these runtime APIs without requiring to import
//! the native runtimes.
#![allow(missing_docs)]

use cord_primitives::{AccountId, Balance, Block, Nonce};
use pallet_transaction_payment::{FeeDetails, RuntimeDispatchInfo};
pub use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_consensus_grandpa::AuthorityId as GrandpaId;

use sp_core::OpaqueMetadata;
use sp_runtime::{
	traits::Block as BlockT,
	transaction_validity::{TransactionSource, TransactionValidity},
	ApplyExtrinsicResult,
};
use sp_version::RuntimeVersion;
use sp_weights::Weight;

sp_api::decl_runtime_apis! {
	/// This runtime API is only implemented for the test runtime!
	pub trait GetLastTimestamp {
		/// Returns the last timestamp of a runtime.
		fn get_last_timestamp() -> u64;
	}
}

struct Runtime;

sp_api::impl_runtime_apis! {
	impl sp_api::Core<Block> for Runtime {
		fn version() -> RuntimeVersion {
			unimplemented!()
		}

		fn execute_block(_: Block) {
			unimplemented!()
		}

		fn initialize_block(_: &<Block as BlockT>::Header) -> sp_runtime::ExtrinsicInclusionMode {
			unimplemented!()
		}
	}

	impl sp_api::Metadata<Block> for Runtime {
		fn metadata() -> OpaqueMetadata {
			unimplemented!()
		}

		fn metadata_at_version(_: u32) -> Option<OpaqueMetadata> {
			unimplemented!()
		}

		fn metadata_versions() -> Vec<u32> {
			unimplemented!()
		}
	}

	impl sp_block_builder::BlockBuilder<Block> for Runtime {
		fn apply_extrinsic(_: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
			unimplemented!()
		}

		fn finalize_block() -> <Block as BlockT>::Header {
			unimplemented!()
		}

		fn inherent_extrinsics(_: sp_inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
			unimplemented!()
		}

		fn check_inherents(
			_: Block,
			_: sp_inherents::InherentData,
		) -> sp_inherents::CheckInherentsResult {
			unimplemented!()
		}
	}

	impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
		fn validate_transaction(
			_: TransactionSource,
			_: <Block as BlockT>::Extrinsic,
			_: <Block as BlockT>::Hash,
		) -> TransactionValidity {
			unimplemented!()
		}
	}

	impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
		fn offchain_worker(_: &<Block as BlockT>::Header) {
			unimplemented!()
		}
	}

	impl sp_consensus_grandpa::GrandpaApi<Block> for Runtime {
		fn grandpa_authorities() -> Vec<(GrandpaId, u64)> {
			unimplemented!()
		}

		fn current_set_id() -> sp_consensus_grandpa::SetId {
			unimplemented!()
		}

		fn submit_report_equivocation_unsigned_extrinsic(
			_: sp_consensus_grandpa::EquivocationProof<
				<Block as BlockT>::Hash,
				sp_runtime::traits::NumberFor<Block>,
			>,
			_: sp_consensus_grandpa::OpaqueKeyOwnershipProof,
		) -> Option<()> {
			unimplemented!()
		}

		fn generate_key_ownership_proof(
			_: sp_consensus_grandpa::SetId,
			_: sp_consensus_grandpa::AuthorityId,
		) -> Option<sp_consensus_grandpa::OpaqueKeyOwnershipProof> {
			unimplemented!()
		}
	}

	impl sp_consensus_babe::BabeApi<Block> for Runtime {
		fn configuration() -> sp_consensus_babe::BabeConfiguration {
			unimplemented!()
		}

		fn current_epoch_start() -> sp_consensus_babe::Slot {
			unimplemented!()
		}

		fn current_epoch() -> sp_consensus_babe::Epoch {
			unimplemented!()
		}

		fn next_epoch() -> sp_consensus_babe::Epoch {
			unimplemented!()
		}

		fn generate_key_ownership_proof(
			_: sp_consensus_babe::Slot,
			_: sp_consensus_babe::AuthorityId,
		) -> Option<sp_consensus_babe::OpaqueKeyOwnershipProof> {
			unimplemented!()
		}

		fn submit_report_equivocation_unsigned_extrinsic(
			_: sp_consensus_babe::EquivocationProof<<Block as BlockT>::Header>,
			_: sp_consensus_babe::OpaqueKeyOwnershipProof,
		) -> Option<()> {
			unimplemented!()
		}
	}

	impl sp_authority_discovery::AuthorityDiscoveryApi<Block> for Runtime {
		fn authorities() -> Vec<AuthorityDiscoveryId> {
			unimplemented!()
		}
	}

	impl sp_session::SessionKeys<Block> for Runtime {
		fn generate_session_keys(_: Option<Vec<u8>>) -> Vec<u8> {
			unimplemented!()
		}

		fn decode_session_keys(
			_: Vec<u8>,
		) -> Option<Vec<(Vec<u8>, sp_core::crypto::KeyTypeId)>> {
			unimplemented!()
		}
	}

	impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Nonce> for Runtime {
		fn account_nonce(_: AccountId) -> Nonce {
			unimplemented!()
		}
	}

	impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<
		Block,
		Balance,
	> for Runtime {
		fn query_info(_: <Block as BlockT>::Extrinsic, _: u32) -> RuntimeDispatchInfo<Balance> {
			unimplemented!()
		}
		fn query_fee_details(_: <Block as BlockT>::Extrinsic, _: u32) -> FeeDetails<Balance> {
			unimplemented!()
		}
		fn query_weight_to_fee(_: Weight) -> Balance {
			unimplemented!()
		}
		fn query_length_to_fee(_: u32) -> Balance {
			unimplemented!()
		}
	}

	impl crate::fake_runtime_api::GetLastTimestamp<Block> for Runtime {
		fn get_last_timestamp() -> u64 {
			unimplemented!()
		}
	}

}
