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

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod weights;

pub use self::currency::MILLI;

/// Money matters.
pub mod currency {
	use polkadot_primitives::Balance;

	/// The existential deposit.
	pub const EXISTENTIAL_DEPOSIT: Balance = 100 * MILLI;

	pub const UNITS: Balance = 10_000_000_000; // 10¹⁰
	pub const MICRO: Balance = UNITS / 100;
	pub const MILLI: Balance = UNITS / 1_000;
	pub const NANO: Balance = UNITS / 10_000;
	pub const GRAND: Balance = UNITS * 1_000; // 10¹³

	pub const fn deposit(items: u32, bytes: u32) -> Balance {
		items as Balance * 10 * UNITS + (bytes as Balance) * 100 * MILLI
	}
}

/// Time and blocks.
pub mod time {
	use origin_common::prod_or_fast;
	use polkadot_primitives::{BlockNumber, Moment};
	pub const MILLISECS_PER_BLOCK: Moment = 6000;
	pub const SLOT_DURATION: Moment = MILLISECS_PER_BLOCK;
	pub const EPOCH_DURATION_IN_SLOTS: BlockNumber = prod_or_fast!(2 * MINUTES, MINUTES);

	// These time units are defined in number of blocks.
	pub const MINUTES: BlockNumber = 60_000 / (MILLISECS_PER_BLOCK as BlockNumber);
	pub const HOURS: BlockNumber = MINUTES * 60;
	pub const DAYS: BlockNumber = HOURS * 24;
	pub const WEEKS: BlockNumber = DAYS * 7;

	// 1 in 4 blocks (on average, not counting collisions) will be primary babe blocks.
	// The choice of is done in accordance to the slot duration and expected target
	// block time, for safely resisting network delays of maximum two seconds.
	// <https://research.web3.foundation/en/latest/polkadot/BABE/Babe/#6-practical-results>
	pub const PRIMARY_PROBABILITY: (u64, u64) = (1, 4);
}

/// Fee-related.
pub mod fee {
	use crate::weights::ExtrinsicBaseWeight;
	use frame_support::weights::{
		WeightToFeeCoefficient, WeightToFeeCoefficients, WeightToFeePolynomial,
	};
	use polkadot_primitives::Balance;
	use smallvec::smallvec;
	pub use sp_runtime::Perbill;

	/// The block saturation level. Fees will be updates based on this value.
	pub const TARGET_BLOCK_FULLNESS: Perbill = Perbill::from_percent(25);

	/// Cost of every transaction byte at Origin relay chain.
	pub const TRANSACTION_BYTE_FEE: Balance = 10 * super::currency::MILLI;

	/// Handles converting a weight scalar to a fee value, based on the scale and granularity of the
	/// node's balance type.
	///
	/// This should typically create a mapping between the following ranges:
	///   - [0, `MAXIMUM_BLOCK_WEIGHT`]
	///   - [Balance::min, Balance::max]
	///
	/// Yet, it can be used for any other sort of change to weight-fee. Some examples being:
	///   - Setting it to `0` will essentially disable the weight fee.
	///   - Setting it to `1` will cause the literal `#[weight = x]` values to be charged.
	pub struct WeightToFee;
	impl WeightToFeePolynomial for WeightToFee {
		type Balance = Balance;
		fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
			// in Polkadot, extrinsic base weight (smallest non-zero weight) is mapped to 1/10
			// MILLI:
			let p = super::currency::MILLI;
			let q = 10 * Balance::from(ExtrinsicBaseWeight::get().ref_time());
			smallvec![WeightToFeeCoefficient {
				degree: 1,
				negative: false,
				coeff_frac: Perbill::from_rational(p % q, q),
				coeff_integer: p / q,
			}]
		}
	}
}

/// System Parachains.
pub mod system_parachain {
	use frame_support::parameter_types;
	use polkadot_primitives::Id as ParaId;
	use xcm_builder::IsChildSystemParachain;

	parameter_types! {
		pub OriginHubInParaId: ParaId = ORIGIN_HUB_IN_ID.into();
		pub OriginHubNaParaId: ParaId = ORIGIN_HUB_NA_ID.into();
		pub OriginHubEuParaId: ParaId = ORIGIN_HUB_EU_ID.into();
		pub OriginHubAPParaId: ParaId = ORIGIN_HUB_AP_ID.into();
		pub OriginHubMeParaId: ParaId = ORIGIN_HUB_ME_ID.into();
		pub OriginHubAfParaId: ParaId = ORIGIN_HUB_AF_ID.into();
	}

	/// Origin Hub parachain IDs.
	pub const ORIGIN_HUB_IN_ID: u32 = 1000;
	pub const ORIGIN_HUB_NA_ID: u32 = 1001;
	pub const ORIGIN_HUB_EU_ID: u32 = 1002;
	pub const ORIGIN_HUB_AP_ID: u32 = 1003;
	pub const ORIGIN_HUB_ME_ID: u32 = 1004;
	pub const ORIGIN_HUB_AF_ID: u32 = 1005;

	// System parachains from Polkadot point of view.
	pub type SystemParachains = IsChildSystemParachain<ParaId>;

	/// Coretime constants
	pub mod coretime {
		/// Coretime timeslice period in blocks
		/// WARNING: This constant is used across chains, so additional care should be taken
		/// when changing it.
		#[cfg(feature = "fast-runtime")]
		pub const TIMESLICE_PERIOD: u32 = 20;
		#[cfg(not(feature = "fast-runtime"))]
		pub const TIMESLICE_PERIOD: u32 = 80;
	}
}

/// Polkadot Treasury pallet instance.
pub const TREASURY_PALLET_ID: u8 = 19;

pub mod proxy {
	use pallet_remote_proxy::ProxyDefinition;
	use polkadot_primitives::{AccountId, BlakeTwo256, BlockNumber, Hash};
	use sp_runtime::traits::Convert;

	/// The type used to represent the kinds of proxying allowed.
	#[derive(
		Copy,
		Clone,
		Eq,
		PartialEq,
		Ord,
		PartialOrd,
		codec::Encode,
		codec::Decode,
		codec::DecodeWithMemTracking,
		codec::MaxEncodedLen,
		core::fmt::Debug,
		scale_info::TypeInfo,
		Default,
	)]
	pub enum ProxyType {
		#[default]
		Any = 0,
		NonTransfer = 1,
		Governance = 2,
		Staking = 3,
		// Skip 4 as it is now removed (was SudoBalances)
		// Skip 5 as it was IdentityJudgement
		CancelProxy = 6,
		Auction = 7,
		NominationPools = 8,
		ParaRegistration = 9,
	}

	/// Remote proxy interface that uses the relay chain as remote location.
	pub struct RemoteProxyInterface<LocalProxyType, ProxyDefinitionConverter>(
		core::marker::PhantomData<(LocalProxyType, ProxyDefinitionConverter)>,
	);

	impl<
			LocalProxyType,
			ProxyDefinitionConverter: Convert<
				ProxyDefinition<AccountId, ProxyType, BlockNumber>,
				Option<ProxyDefinition<AccountId, LocalProxyType, BlockNumber>>,
			>,
		> pallet_remote_proxy::RemoteProxyInterface<AccountId, LocalProxyType, BlockNumber>
		for RemoteProxyInterface<LocalProxyType, ProxyDefinitionConverter>
	{
		type RemoteAccountId = AccountId;

		type RemoteProxyType = ProxyType;

		type RemoteBlockNumber = BlockNumber;

		type RemoteHash = Hash;

		type RemoteHasher = BlakeTwo256;

		fn block_to_storage_root(
			validation_data: &polkadot_primitives::PersistedValidationData,
		) -> Option<(Self::RemoteBlockNumber, <Self::RemoteHasher as sp_core::Hasher>::Out)> {
			Some((validation_data.relay_parent_number, validation_data.relay_parent_storage_root))
		}

		fn local_to_remote_account_id(local: &AccountId) -> Option<Self::RemoteAccountId> {
			Some(local.clone())
		}

		fn remote_to_local_proxy_defintion(
			remote: ProxyDefinition<
				Self::RemoteAccountId,
				Self::RemoteProxyType,
				Self::RemoteBlockNumber,
			>,
		) -> Option<ProxyDefinition<AccountId, LocalProxyType, BlockNumber>> {
			ProxyDefinitionConverter::convert(remote)
		}

		#[cfg(feature = "runtime-benchmarks")]
		fn create_remote_proxy_proof(
			caller: &AccountId,
			proxy: &AccountId,
		) -> (pallet_remote_proxy::RemoteProxyProof<Self::RemoteBlockNumber>, BlockNumber, Hash) {
			use codec::Encode;
			use sp_trie::TrieMut;

			let (mut db, mut root) = sp_trie::MemoryDB::<BlakeTwo256>::default_with_root();
			let mut trie =
				sp_trie::TrieDBMutBuilder::<sp_trie::LayoutV1<_>>::new(&mut db, &mut root).build();

			let proxy_definition =
				alloc::vec![ProxyDefinition::<AccountId, ProxyType, BlockNumber> {
					delegate: caller.clone(),
					proxy_type: ProxyType::default(),
					delay: 0,
				}];

			trie.insert(&Self::proxy_definition_storage_key(proxy), &proxy_definition.encode())
				.unwrap();
			drop(trie);

			(
				pallet_remote_proxy::RemoteProxyProof::RelayChain {
					proof: db.drain().into_values().map(|d| d.0).collect(),
					block: 1,
				},
				1,
				root,
			)
		}
	}
}

#[cfg(test)]
mod tests {
	use super::{
		currency::{MICRO, MILLI, UNITS},
		fee::WeightToFee,
		proxy::ProxyType,
	};
	use crate::weights::ExtrinsicBaseWeight;
	use codec::{Decode, DecodeWithMemTracking, Encode};
	use frame_support::weights::WeightToFee as WeightToFeeT;
	use polkadot_runtime_common::MAXIMUM_BLOCK_WEIGHT;

	#[test]
	// Test that the fee for `MAXIMUM_BLOCK_WEIGHT` of weight has sane bounds.
	fn full_block_fee_is_correct() {
		// A full block should cost between 10 and 100 UNITS.
		let full_block = WeightToFee::weight_to_fee(&MAXIMUM_BLOCK_WEIGHT);
		assert!(full_block >= 10 * UNITS);
		assert!(full_block <= 100 * UNITS);
	}

	#[test]
	// This function tests that the fee for `ExtrinsicBaseWeight` of weight is correct
	fn extrinsic_base_fee_is_correct() {
		// `ExtrinsicBaseWeight` should cost 1/10 of a CENT
		println!("Base: {}", ExtrinsicBaseWeight::get());
		let x = WeightToFee::weight_to_fee(&ExtrinsicBaseWeight::get());
		let y = MILLI / 10;
		assert!(x.max(y) - x.min(y) < MICRO);
	}

	#[derive(
		Copy,
		Clone,
		Eq,
		PartialEq,
		Ord,
		PartialOrd,
		Encode,
		Decode,
		DecodeWithMemTracking,
		sp_runtime::Debug,
	)]
	pub enum OldProxyType {
		Any,
		NonTransfer,
		Governance,
		Staking,
		SudoBalances,
		IdentityJudgement,
	}

	#[test]
	fn proxy_type_decodes_correctly() {
		for (i, j) in vec![
			(OldProxyType::Any, ProxyType::Any),
			(OldProxyType::NonTransfer, ProxyType::NonTransfer),
			(OldProxyType::Governance, ProxyType::Governance),
			(OldProxyType::Staking, ProxyType::Staking),
		]
		.into_iter()
		{
			assert_eq!(i.encode(), j.encode());
		}
		assert!(ProxyType::decode(&mut &OldProxyType::SudoBalances.encode()[..]).is_err());
		assert!(ProxyType::decode(&mut &OldProxyType::IdentityJudgement.encode()[..]).is_err());
	}
}
