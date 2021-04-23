// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

//! Benchmarking of Mark

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use crate::Module as AttestationModule;
use pallet_delegation::{benchmarking::setup_delegations, Permissions};
use frame_benchmarking::benchmarks;
use frame_support::storage::StorageMap;
use frame_system::RawOrigin;
use sp_core::sr25519;
use sp_runtime::traits::Hash;
use sp_std::{num::NonZeroU32,vec};
use frame_support::traits::Box;

const MAX_DEPTH: u32 = 10;
const ONE_CHILD_PER_LEVEL: Option<NonZeroU32> = NonZeroU32::new(1);

benchmarks! {
	where_clause { where T: core::fmt::Debug, T::Signature: From<sr25519::Signature>, T::AccountId: From<sr25519::Public>, 	T::DelegationNodeId: From<T::Hash> }

	anchor {
		let content_hash: T::Hash = T::Hashing::hash(b"claim");
		let mtype_hash: T::Hash = T::Hash::default();
		let (_, _, delegate_public, delegation_id) = setup_delegations::<T>(1, ONE_CHILD_PER_LEVEL.expect(">0"), Permissions::ANCHOR)?;
		let delegate_acc: T::AccountId = delegate_public.into();
	}: _(RawOrigin::Signed(delegate_acc.clone()), content_hash, mtype_hash, Some(delegation_id))
	verify {
		assert!(Marks::<T>::contains_key(content_hash));
		assert_eq!(AttestationModule::<T>::marks(content_hash), Some(Mark::<T> {
			mtype_hash,
			marker: delegate_acc,
			delegation_id: Some(delegation_id),
			revoked: false,
		}));
	}

	// revoke {
	// 	let d in 1 .. MAX_DEPTH;

	// 	let content_hash: T::Hash = T::Hashing::hash(b"claim");
	// 	let mtype_hash: T::Hash = T::Hash::default();

	// 	let (root_public, _, delegate_public, delegation_id) = setup_delegations::<T>(d, ONE_CHILD_PER_LEVEL.expect(">0"), Permissions::ANCHOR | Permissions::DELEGATE)?;
	// 	let root_acc: T::AccountId = root_public.into();
	// 	let delegate_acc: T::AccountId = delegate_public.into();mtype_hash

	// 	// attest with leaf account
	// 	AttestationModule::<T>::anchor(RawOrigin::Signed(delegate_acc.clone()).into(), content_hash, mtype_hash, Some(delegation_id))?;
	// 	// revoke with root account, s.t. delegation tree needs to be traversed
	// }: _(RawOrigin::Signed(root_acc.clone()), claim_hash, d + 1)
	// verify {
	// 	assert!(Marks::<T>::contains_key(content_hash));
	// 	assert_eq!(Attestations::<T>::get(content_hash), Some(Mark::<T> {
	// 		mtype_hash,
	// 		marker: delegate_acc,
	// 		delegation_id: Some(delegation_id),
	// 		revoked: true,
	// 	}));
	// }
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::tests::{ExtBuilder, Test};
	use frame_support::assert_ok;

	#[test]
	fn test_benchmarks() {
		ExtBuilder::build_with_keystore().execute_with(|| {
			assert_ok!(test_benchmark_add::<Test>());
			assert_ok!(test_benchmark_revoke::<Test>());
		});
	}
}