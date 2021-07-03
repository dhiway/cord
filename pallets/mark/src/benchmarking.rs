// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

//! Benchmarking of Mark

#![cfg(feature = "runtime-benchmarks")]

use delegation::{benchmarking::setup_delegations, Permissions};
use frame_benchmarking::{benchmarks, impl_benchmark_test_suite};
use frame_system::RawOrigin;
use sp_core::sr25519;
use sp_runtime::traits::Hash;
use sp_std::num::NonZeroU32;

use crate::*;

const ONE_CHILD_PER_LEVEL: Option<NonZeroU32> = NonZeroU32::new(1);

benchmarks! {
	where_clause { where T: core::fmt::Debug, T::AccountId: From<sr25519::Public> + Into<T::DelegationEntityId>, T::DelegationNodeId: From<T::Hash> }

	anchor {
		let stream_hash: T::Hash = T::Hashing::hash(b"stream");
		let mtype_hash: T::Hash = T::Hash::default();
		let (_, _, delegate_public, delegation_id) = setup_delegations::<T>(1, ONE_CHILD_PER_LEVEL.expect(">0"), Permissions::ANCHOR)?;
		let delegate_acc: T::AccountId = delegate_public.into();
	}: _(RawOrigin::Signed(delegate_acc.clone()), stream_hash, mtype_hash, Some(delegation_id))
	verify {
		assert!(Marks::<T>::contains_key(stream_hash));
		assert_eq!(Pallet::<T>::marks(stream_hash), Some(MarkDetails {
			mtype_hash,
			issuer: delegate_acc,
			delegation_id: Some(delegation_id),
			revoked: false,
		}));
	}

	revoke {
		let d in 1 .. T::MaxParentChecks::get();

		let stream_hash: T::Hash = T::Hashing::hash(b"stream");
		let mtype_hash: T::Hash = T::Hash::default();

		let (root_public, _, delegate_public, delegation_id) = setup_delegations::<T>(d, ONE_CHILD_PER_LEVEL.expect(">0"), Permissions::ANCHOR | Permissions::DELEGATE)?;
		let root_acc: T::AccountId = root_public.into();
		let delegate_acc: T::AccountId = delegate_public.into();

		// attest with leaf account
		Pallet::<T>::anchor(RawOrigin::Signed(delegate_acc.clone()).into(), stream_hash, mtype_hash, Some(delegation_id))?;
		// revoke with root account, s.t. delegation tree needs to be traversed
	}: _(RawOrigin::Signed(root_acc.clone()), stream_hash, d)
	verify {
		assert!(Marks::<T>::contains_key(stream_hash));
		assert_eq!(Marks::<T>::get(stream_hash), Some(MarkDetails {
			mtype_hash,
			issuer: delegate_acc,
			delegation_id: Some(delegation_id),
			revoked: true,
		}));
	}

	// restore {
	// 	let d in 1 .. T::MaxParentChecks::get();

	// 	let stream_hash: T::Hash = T::Hashing::hash(b"stream");
	// 	let mtype_hash: T::Hash = T::Hash::default();

	// 	let (root_public, _, delegate_public, delegation_id) = setup_delegations::<T>(d, ONE_CHILD_PER_LEVEL.expect(">0"), Permissions::ANCHOR | Permissions::DELEGATE)?;
	// 	let root_acc: T::AccountId = root_public.into();
	// 	let delegate_acc: T::AccountId = delegate_public.into();

	// 	// attest with leaf account
	// 	Pallet::<T>::anchor(RawOrigin::Signed(delegate_acc.clone()).into(), stream_hash, mtype_hash, Some(delegation_id))?;
	// 	Pallet::<T>::revoke(RawOrigin::Signed(root_acc.clone()).into(), stream_hash,d+1)?;

	// 	// revoke with root account, s.t. delegation tree needs to be traversed
	// }: _(RawOrigin::Signed(root_acc.clone()), stream_hash, d)
	// verify {
	// 	assert!(Marks::<T>::contains_key(stream_hash));
	// 	assert_eq!(Marks::<T>::get(stream_hash), Some(Mark::<T> {
	// 		mtype_hash,
	// 		issuer: delegate_acc,
	// 		delegation_id: Some(delegation_id),
	// 		revoked: false,
	// 	}));
	// }

}

impl_benchmark_test_suite! {
	Pallet,
	crate::mock::ExtBuilder::default().build_with_keystore(None),
	crate::mock::Test
}
