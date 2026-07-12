// Copyright (C) Parity Technologies (UK) Ltd.
// This file is part of Individuality.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use super::*;
use crate::extension::{AsPerson, AsPersonInfo};
use alloc::{vec, vec::Vec};
use core::marker::{Send, Sync};
use frame_benchmarking::{account, v2::*, BenchmarkError};
use frame_support::{
	assert_ok,
	dispatch::RawOrigin,
	pallet_prelude::{Authorize, BoundedVec, ConstU32, Pays},
	traits::{EnsureOrigin, Get},
};
use frame_system::RawOrigin as SystemOrigin;
use indiv_support::traits::{AppendOnlyMembers, Context, ContextualAlias, RingMode};
use sp_runtime::{
	generic::ExtensionVersion,
	traits::{AppendZerosInput, AsTransactionAuthorizedOrigin, DispatchTransaction},
	transaction_validity::TransactionSource,
	Weight,
};
use verifiable::Alias;

const SEED: u32 = 0;

fn new_member_from<T: Config + Send + Sync>(i: u32, seed: u32) -> (SecretOf<T>, MemberOf<T>) {
	let mut entropy = &(i, seed).encode()[..];
	let mut entropy = AppendZerosInput::new(&mut entropy);
	let secret = CryptoOf::<T>::new_secret(Decode::decode(&mut entropy).unwrap());
	let public = CryptoOf::<T>::member_from_secret(&secret);
	(secret, public)
}

fn generate_members<T: Config + Send + Sync>(
	seed: u32,
	start: u32,
	end: u32,
) -> Vec<(SecretOf<T>, MemberOf<T>)> {
	(start..end).map(|i| new_member_from::<T>(i, seed)).collect::<Vec<_>>()
}

fn max_ring_size<T: Config>() -> u32 {
	T::RingExponent::get().ring_capacity()
}

fn generate_members_for_ring<T: Config + Send + Sync>(
	seed: u32,
) -> Vec<(SecretOf<T>, MemberOf<T>)> {
	(0..max_ring_size::<T>())
		.map(|i| new_member_from::<T>(i, seed))
		.collect::<Vec<_>>()
}

pub fn recognize_people<T: Config + Send + Sync>(
	members: &[(SecretOf<T>, MemberOf<T>)],
) -> Vec<(PersonalId, MemberOf<T>, SecretOf<T>)> {
	let mut people = Vec::new();
	for (secret, public) in members.iter() {
		let person = pallet::Pallet::<T>::reserve_new_id();
		pallet::Pallet::<T>::recognize_personhood(person, Some(public.clone())).unwrap();
		people.push((person, public.clone(), secret.clone()));
	}

	people
}

pub trait BenchmarkHelper<Chunk> {
	fn valid_account_context() -> Context;
	/// Returns a valid context that exercises the worst-case path through
	/// `AccountContexts::contains` (all static checks miss, then storage read).
	/// Implementations should set up any necessary state (e.g. active airdrop events).
	fn worst_case_account_context() -> Context {
		Self::valid_account_context()
	}
	fn initialize_chunks() -> Vec<Chunk>;
}

#[cfg(feature = "std")]
impl BenchmarkHelper<()> for () {
	fn valid_account_context() -> Context {
		[0u8; 32]
	}

	fn initialize_chunks() -> Vec<()> {
		vec![]
	}
}

/// Set up the people ring. This function initializes the chunks required for the ZK crypto, creates
/// the people collection, recognizes the provided members, onboards them then builds the ring.
fn setup_people_ring<T: Config + Send + Sync>(members: &[(SecretOf<T>, MemberOf<T>)]) {
	T::MemberService::create_collection(
		Decode::decode(&mut &[0u8; 32][..]).unwrap(),
		PEOPLE_MEMBER_IDENTIFIER,
		1,
		RingMode::Flexible,
		T::RingExponent::get(),
		None,
	)
	.unwrap();

	recognize_people::<T>(members);

	T::MemberService::initialize_chunks(T::RingExponent::get());
	T::MemberService::onboard_all_and_build_ring(PEOPLE_MEMBER_IDENTIFIER, 0).unwrap();
}

/// Create a ring VRF proof for a member.
fn create_ring_proof<T: Config + Send + Sync>(
	secret: &SecretOf<T>,
	member: &MemberOf<T>,
	context: &[u8],
	msg: &[u8],
) -> (ProofOf<T>, Alias) {
	let ring_members = T::MemberService::ring_members(PEOPLE_MEMBER_IDENTIFIER, 0);
	let capacity: <CryptoOf<T> as GenerateVerifiable>::Config = T::RingExponent::get()
		.try_into()
		.ok()
		.expect("ring exponent must convert to capacity");
	let commitment =
		CryptoOf::<T>::open(capacity, member, ring_members.into_iter()).expect("open must succeed");
	CryptoOf::<T>::create(commitment, secret, context, msg).expect("create must succeed")
}

#[benchmarks(
	where T: Send + Sync,
		<T as frame_system::Config>::RuntimeCall:
			Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo> + IsSubType<Call<T>> + From<Call<T>> + GetDispatchInfo,
		<T as frame_system::Config>::RuntimeOrigin: AsTransactionAuthorizedOrigin,
)]
mod benches {
	use super::*;

	#[benchmark]
	fn under_alias() -> Result<(), BenchmarkError> {
		let members = generate_members_for_ring::<T>(SEED);
		setup_people_ring::<T>(&members);

		let caller: T::AccountId = account("caller", 0, SEED);
		let context = T::BenchmarkHelper::worst_case_account_context();
		let (secret, _public) = &members[0];

		let alias_value = CryptoOf::<T>::alias_in_context(secret, &context[..])
			.expect("alias creation must succeed");
		let revision = T::MemberService::ring_revision(PEOPLE_MEMBER_IDENTIFIER, 0)
			.ok_or(BenchmarkError::Stop("people ring revision missing"))?;
		let alias = RevisedContextualAlias {
			ca: ContextualAlias { context, alias: alias_value },
			revision,
			ring: 0,
		};
		let block_number = frame_system::Pallet::<T>::block_number();
		assert_ok!(pallet::Pallet::<T>::set_alias_account(
			Origin::PersonalAlias(alias.clone()).into(),
			caller.clone(),
			block_number,
		));

		// The derivative call to dispatch under the alias.
		let inner_call: <T as frame_system::Config>::RuntimeCall =
			frame_system::Call::<T>::remark { remark: vec![] }.into();

		#[extrinsic_call]
		_(SystemOrigin::Signed(caller.clone()), Box::new(inner_call));

		frame_system::Pallet::<T>::assert_last_event(
			crate::Event::<T>::AliasDispatched { alias: alias.ca, account: caller }.into(),
		);

		Ok(())
	}

	#[benchmark]
	fn set_alias_account() -> Result<(), BenchmarkError> {
		// Set up the member collection
		T::MemberService::create_collection(
			Decode::decode(&mut &[0u8; 32][..]).unwrap(),
			PEOPLE_MEMBER_IDENTIFIER,
			max_ring_size::<T>(),
			RingMode::Flexible,
			T::RingExponent::get(),
			None,
		)
		.map_err(|_| BenchmarkError::Stop("failed to create collection"))?;

		// Generate people and add them
		let members = generate_members_for_ring::<T>(SEED);
		let people = recognize_people::<T>(&members);

		let block_number = frame_system::Pallet::<T>::block_number();
		// Use worst-case context to capture the storage read from dynamic context
		// validation (e.g. airdrop contexts) in the benchmark weight.
		let context = T::BenchmarkHelper::worst_case_account_context();
		let (_, _, secret) = &people[0];

		let alias_value = CryptoOf::<T>::alias_in_context(secret, &context[..])
			.expect("alias creation must succeed");
		let alias = RevisedContextualAlias {
			ca: ContextualAlias { context, alias: alias_value },
			revision: 0,
			ring: 0,
		};

		// An account had already been assigned to this alias
		let old_account: T::AccountId = account("test_old", 0, SEED);
		assert_ok!(pallet::Pallet::<T>::set_alias_account(
			Origin::PersonalAlias(alias.clone()).into(),
			old_account.clone(),
			block_number
		));
		assert!(AccountToAlias::<T>::contains_key(&old_account));
		assert!(AliasToAccount::<T>::contains_key(&alias.ca));

		let account: T::AccountId = account("test", 0, SEED);

		#[extrinsic_call]
		_(Origin::PersonalAlias(alias.clone()), account.clone(), block_number);

		assert!(!AccountToAlias::<T>::contains_key(&old_account));
		assert!(AccountToAlias::<T>::contains_key(&account));
		assert!(AliasToAccount::<T>::contains_key(&alias.ca));
		assert_eq!(AliasToAccount::<T>::get(&alias.ca), Some(account.clone()));

		frame_system::Pallet::<T>::assert_last_event(
			crate::Event::<T>::AliasAccountSet { alias: alias.ca, account }.into(),
		);

		Ok(())
	}

	#[benchmark]
	fn unset_alias_account() -> Result<(), BenchmarkError> {
		// Set up the member collection
		T::MemberService::create_collection(
			Decode::decode(&mut &[0u8; 32][..]).unwrap(),
			PEOPLE_MEMBER_IDENTIFIER,
			max_ring_size::<T>(),
			RingMode::Flexible,
			T::RingExponent::get(),
			None,
		)
		.map_err(|_| BenchmarkError::Stop("failed to create collection"))?;

		// Generate people and add them
		let members = generate_members_for_ring::<T>(SEED);
		let people = recognize_people::<T>(&members);

		let account: T::AccountId = account("test", 0, SEED);
		let block_number = frame_system::Pallet::<T>::block_number();
		let context = T::BenchmarkHelper::worst_case_account_context();
		let (_, _, secret) = &people[0];

		let alias_value = CryptoOf::<T>::alias_in_context(secret, &context[..])
			.expect("alias creation must succeed");
		let alias = RevisedContextualAlias {
			ca: ContextualAlias { context, alias: alias_value },
			revision: 0,
			ring: 0,
		};

		assert_ok!(pallet::Pallet::<T>::set_alias_account(
			Origin::PersonalAlias(alias.clone()).into(),
			account.clone(),
			block_number
		));
		assert!(AccountToAlias::<T>::contains_key(&account));
		assert!(AliasToAccount::<T>::contains_key(&alias.ca));

		#[extrinsic_call]
		_(Origin::PersonalAlias(alias.clone()));

		assert!(!AccountToAlias::<T>::contains_key(&account));
		assert!(!AliasToAccount::<T>::contains_key(&alias.ca));

		frame_system::Pallet::<T>::assert_last_event(
			crate::Event::<T>::AliasAccountUnset { alias: alias.ca, account }.into(),
		);

		Ok(())
	}

	#[benchmark]
	fn force_recognize_personhood(
		n: Linear<1, { max_ring_size::<T>() }>,
	) -> Result<(), BenchmarkError> {
		// Set up the member collection
		T::MemberService::create_collection(
			Decode::decode(&mut &[0u8; 32][..]).unwrap(),
			PEOPLE_MEMBER_IDENTIFIER,
			max_ring_size::<T>(),
			RingMode::Flexible,
			T::RingExponent::get(),
			None,
		)
		.map_err(|_| BenchmarkError::Stop("failed to create collection"))?;

		let members = generate_members::<T>(SEED, 0, n);
		let payload: Vec<MemberOf<T>> = members.iter().map(|(_, m)| m.clone()).collect();

		let origin =
			T::ManagerOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;

		#[extrinsic_call]
		_(origin, payload.clone());

		for person in members {
			assert!(pallet::Keys::<T>::get(person.1).is_some());
		}

		frame_system::Pallet::<T>::assert_last_event(
			crate::Event::<T>::ForcePersonhoodRecognized { people: payload }.into(),
		);

		Ok(())
	}

	#[benchmark]
	fn set_personal_id_account() -> Result<(), BenchmarkError> {
		// Set up the member collection
		T::MemberService::create_collection(
			Decode::decode(&mut &[0u8; 32][..]).unwrap(),
			PEOPLE_MEMBER_IDENTIFIER,
			max_ring_size::<T>(),
			RingMode::Flexible,
			T::RingExponent::get(),
			None,
		)
		.map_err(|_| BenchmarkError::Stop("failed to create collection"))?;

		// Generate people and add them
		let members = generate_members_for_ring::<T>(SEED);
		let people = recognize_people::<T>(&members);

		// Get one of the generated people's information
		let (personal_id, _, _): &(PersonalId, MemberOf<T>, SecretOf<T>) = &people[0];

		let account: T::AccountId = account("test", 0, SEED);
		let block_number = frame_system::Pallet::<T>::block_number();

		// An account had already been assigned to this personal id
		let old_account: T::AccountId = frame_benchmarking::account("test_old", 0, SEED);
		assert_ok!(pallet::Pallet::<T>::set_personal_id_account(
			Origin::PersonalIdentity(*personal_id).into(),
			old_account.clone(),
			block_number
		));

		#[extrinsic_call]
		_(Origin::PersonalIdentity(*personal_id), account.clone(), block_number);

		assert_eq!(AccountToPersonalId::<T>::get(&old_account), None);
		assert_eq!(AccountToPersonalId::<T>::get(&account), Some(*personal_id));
		assert!(People::<T>::get(personal_id).is_some());
		assert_eq!(People::<T>::get(personal_id).unwrap().account, Some(account.clone()));

		frame_system::Pallet::<T>::assert_last_event(
			crate::Event::<T>::PersonalIdAccountSet { who: *personal_id, account }.into(),
		);

		Ok(())
	}

	#[benchmark]
	fn unset_personal_id_account() -> Result<(), BenchmarkError> {
		// Set up the member collection
		T::MemberService::create_collection(
			Decode::decode(&mut &[0u8; 32][..]).unwrap(),
			PEOPLE_MEMBER_IDENTIFIER,
			max_ring_size::<T>(),
			RingMode::Flexible,
			T::RingExponent::get(),
			None,
		)
		.map_err(|_| BenchmarkError::Stop("failed to create collection"))?;

		// Generate people and add them
		let members = generate_members_for_ring::<T>(SEED);
		let people = recognize_people::<T>(&members);

		// Get one of the generated people's information
		let (personal_id, _, _): &(PersonalId, MemberOf<T>, SecretOf<T>) = &people[0];

		let account: T::AccountId = account("test", 0, SEED);
		let block_number = frame_system::Pallet::<T>::block_number();

		// Set up account association
		assert_ok!(pallet::Pallet::<T>::set_personal_id_account(
			Origin::PersonalIdentity(*personal_id).into(),
			account.clone(),
			block_number
		));
		assert_eq!(AccountToPersonalId::<T>::get(&account), Some(*personal_id));

		#[extrinsic_call]
		_(Origin::PersonalIdentity(*personal_id));

		assert_eq!(AccountToPersonalId::<T>::get(&account), None);
		assert!(People::<T>::get(personal_id).is_some());
		assert_eq!(People::<T>::get(personal_id).unwrap().account, None);

		frame_system::Pallet::<T>::assert_last_event(
			crate::Event::<T>::PersonalIdAccountUnset { who: *personal_id, account }.into(),
		);

		Ok(())
	}

	#[benchmark]
	fn as_person_alias_with_account() -> Result<(), BenchmarkError> {
		let members = generate_members_for_ring::<T>(SEED);
		setup_people_ring::<T>(&members);

		let caller: T::AccountId = account("caller", 0, SEED);
		let context = T::BenchmarkHelper::worst_case_account_context();
		let (secret, _) = &members[0];

		let alias_value = CryptoOf::<T>::alias_in_context(secret, &context[..])
			.expect("alias creation must succeed");
		let revision = T::MemberService::ring_revision(PEOPLE_MEMBER_IDENTIFIER, 0)
			.ok_or(BenchmarkError::Stop("people ring revision missing"))?;
		let alias = RevisedContextualAlias {
			ca: ContextualAlias { context, alias: alias_value },
			revision,
			ring: 0,
		};

		let block_number = frame_system::Pallet::<T>::block_number();
		assert_ok!(pallet::Pallet::<T>::set_alias_account(
			Origin::PersonalAlias(alias.clone()).into(),
			caller.clone(),
			block_number,
		));

		// A simple call to benchmark with.
		let inner = frame_system::Call::<T>::remark { remark: vec![] };
		let call: <T as frame_system::Config>::RuntimeCall = inner.into();

		let ext =
			AsPerson::new(Some(AsPersonInfo::<T>::AsPersonalAliasWithAccount(T::Nonce::default())));
		let info = call.get_dispatch_info();
		let post_info = PostDispatchInfo {
			actual_weight: Some(Weight::from_parts(10, 0)),
			pays_fee: Pays::Yes,
		};
		let len = call.encoded_size();

		#[block]
		{
			ext.test_run(RawOrigin::Signed(caller.clone()).into(), &call, &info, len, 0, |_| {
				Ok(post_info)
			})
			.unwrap()
			.unwrap();
		}

		assert_eq!(frame_system::Pallet::<T>::account_nonce(&caller), 1u32.into());

		Ok(())
	}

	#[benchmark]
	fn as_person_identity_with_account() -> Result<(), BenchmarkError> {
		// Set up the member collection
		T::MemberService::create_collection(
			Decode::decode(&mut &[0u8; 32][..]).unwrap(),
			PEOPLE_MEMBER_IDENTIFIER,
			max_ring_size::<T>(),
			RingMode::Flexible,
			T::RingExponent::get(),
			None,
		)
		.map_err(|_| BenchmarkError::Stop("failed to create collection"))?;

		// Generate people and add them
		let members = generate_members_for_ring::<T>(SEED);
		let recognized_people = recognize_people::<T>(&members);

		// Select one of the generated people's information
		let (personal_id, _, _): &(PersonalId, MemberOf<T>, SecretOf<T>) = &recognized_people[0];

		// Set up personal ID account association
		let account: T::AccountId = account("caller", 0, SEED);
		let block_number = frame_system::Pallet::<T>::block_number();
		assert_ok!(pallet::Pallet::<T>::set_personal_id_account(
			Origin::PersonalIdentity(*personal_id).into(),
			account.clone(),
			block_number
		));
		assert!(AccountToPersonalId::<T>::contains_key(&account));

		// A simple call to benchmark with
		let inner = frame_system::Call::<T>::remark { remark: vec![] };
		let call: <T as frame_system::Config>::RuntimeCall = inner.into();

		let ext = AsPerson::new(Some(AsPersonInfo::<T>::AsPersonalIdentityWithAccount(
			T::Nonce::default(),
		)));
		let info = call.get_dispatch_info();
		let post_info = PostDispatchInfo {
			actual_weight: Some(Weight::from_parts(10, 0)),
			pays_fee: Pays::Yes,
		};
		let len = call.encoded_size();

		#[block]
		{
			ext.test_run(RawOrigin::Signed(account.clone()).into(), &call, &info, len, 0, |_| {
				Ok(post_info)
			})
			.unwrap()
			.unwrap();
		}

		assert_eq!(frame_system::Pallet::<T>::account_nonce(&account), 1u32.into());

		Ok(())
	}

	#[benchmark]
	fn as_person_alias_with_proof() -> Result<(), BenchmarkError> {
		let members = generate_members_for_ring::<T>(SEED);
		setup_people_ring::<T>(&members);

		let caller: T::AccountId = account("caller", 0, SEED);
		let context = T::BenchmarkHelper::worst_case_account_context();
		let (secret, public) = &members[0];

		// The `[0xff; 32]` alias differs from any real bandersnatch-derived alias,
		// so equality is guaranteed false and validate falls through to the success
		// branch.
		let stale_rev_ca = RevisedContextualAlias {
			revision: 0,
			ring: 0,
			ca: ContextualAlias { alias: [0xffu8; 32], context },
		};
		AccountToAlias::<T>::insert(&caller, &stale_rev_ca);

		// The call to set the alias account — the only one valid for this extension code path.
		let block_number = frame_system::Pallet::<T>::block_number();
		let inner =
			Call::<T>::set_alias_account { account: caller.clone(), call_valid_at: block_number };
		let call: <T as frame_system::Config>::RuntimeCall = inner.into();

		// Create the ring VRF proof.
		let ext_version: ExtensionVersion = 0;
		let msg = (ext_version, &call).using_encoded(sp_io::hashing::blake2_256);
		let (proof, _alias) = create_ring_proof::<T>(secret, public, &context[..], &msg);

		let ext =
			AsPerson::new(Some(AsPersonInfo::<T>::AsPersonalAliasWithProof(proof, 0, context)));
		let info = call.get_dispatch_info();
		let post_info = PostDispatchInfo {
			actual_weight: Some(Weight::from_parts(10, 0)),
			pays_fee: Pays::Yes,
		};
		let len = call.encoded_size();

		#[block]
		{
			ext.test_run(RawOrigin::None.into(), &call, &info, len, 0, |_| Ok(post_info))
				.unwrap()
				.unwrap();
		}

		Ok(())
	}

	#[benchmark]
	fn as_person_identity_with_proof() -> Result<(), BenchmarkError> {
		// Set up the member collection
		T::MemberService::create_collection(
			Decode::decode(&mut &[0u8; 32][..]).unwrap(),
			PEOPLE_MEMBER_IDENTIFIER,
			max_ring_size::<T>(),
			RingMode::Flexible,
			T::RingExponent::get(),
			None,
		)
		.map_err(|_| BenchmarkError::Stop("failed to create collection"))?;

		// Generate people and add them
		let caller: T::AccountId = account("caller", 0, SEED);
		let members = generate_members_for_ring::<T>(SEED);
		let recognized_people = recognize_people::<T>(&members);

		// Select one of the generated people's information
		let (personal_id, _, secret): &(PersonalId, MemberOf<T>, SecretOf<T>) =
			&recognized_people[0];

		// traverse the full `record.account.is_some_and(|stored| stored == *account)` path.
		People::<T>::mutate(*personal_id, |maybe| {
			if let Some(record) = maybe {
				record.account = Some(account("other", 0, SEED));
			}
		});

		// The call to set the personal ID account, the only one valid for this extension code
		// path.
		let block_number = frame_system::Pallet::<T>::block_number();
		let inner =
			Call::<T>::set_personal_id_account { account: caller, call_valid_at: block_number };
		let call: <T as frame_system::Config>::RuntimeCall = inner.into();
		let ext_version: ExtensionVersion = 0;
		let signature = (ext_version, &call).using_encoded(|msg| {
			CryptoOf::<T>::sign(secret, &sp_io::hashing::blake2_256(msg))
				.expect("failed to create signature")
		});

		let ext = AsPerson::new(Some(AsPersonInfo::<T>::AsPersonalIdentityWithProof(
			signature,
			*personal_id,
		)));
		let info = call.get_dispatch_info();
		let post_info = PostDispatchInfo {
			actual_weight: Some(Weight::from_parts(10, 0)),
			pays_fee: Pays::Yes,
		};
		let len = call.encoded_size();

		#[block]
		{
			ext.test_run(RawOrigin::None.into(), &call, &info, len, 0, |_| Ok(post_info))
				.unwrap()
				.unwrap();
		}

		Ok(())
	}

	#[benchmark]
	fn as_person_alias_with_account_revised() -> Result<(), BenchmarkError> {
		// Phase 1: Create collection with a subset of people, build ring at revision 0.
		T::MemberService::create_collection(
			Decode::decode(&mut &[0u8; 32][..]).unwrap(),
			PEOPLE_MEMBER_IDENTIFIER,
			// Use an onboarding size of 1 so that `onboard_members` can process any batch.
			1,
			RingMode::Flexible,
			T::RingExponent::get(),
			None,
		)
		.map_err(|_| BenchmarkError::Stop("failed to create collection"))?;

		let all_members = generate_members_for_ring::<T>(SEED);
		let half = all_members.len() / 2;
		let first_half = &all_members[..half];
		let second_half = &all_members[half..];

		recognize_people::<T>(first_half);

		T::MemberService::initialize_chunks(T::RingExponent::get());
		T::MemberService::onboard_all_and_build_ring(PEOPLE_MEMBER_IDENTIFIER, 0)
			.map_err(|_| BenchmarkError::Stop("failed to build ring (phase 1)"))?;

		// Phase 2: Set up alias-account mapping at revision 0.
		let caller: T::AccountId = account("caller", 0, SEED);
		let context = T::BenchmarkHelper::worst_case_account_context();
		let (secret, public) = &first_half[0];

		let alias_value = CryptoOf::<T>::alias_in_context(secret, &context[..])
			.expect("alias creation must succeed");
		let alias = RevisedContextualAlias {
			ca: ContextualAlias { context, alias: alias_value },
			revision: 0,
			ring: 0,
		};
		let block_number = frame_system::Pallet::<T>::block_number();
		assert_ok!(pallet::Pallet::<T>::set_alias_account(
			Origin::PersonalAlias(alias.clone()).into(),
			caller.clone(),
			block_number,
		));

		// Phase 3: Recognize remaining people and rebuild the ring → revision 1.
		recognize_people::<T>(second_half);
		T::MemberService::onboard_all_and_build_ring(PEOPLE_MEMBER_IDENTIFIER, 0)
			.map_err(|_| BenchmarkError::Stop("failed to build ring (phase 2)"))?;
		let new_revision = T::MemberService::ring_revision(PEOPLE_MEMBER_IDENTIFIER, 0)
			.ok_or(BenchmarkError::Stop("people ring revision missing"))?;

		// Phase 4: Create a proof against the updated ring for the revision update.
		let nonce = T::Nonce::default();
		let inner = frame_system::Call::<T>::remark { remark: vec![] };
		let call: <T as frame_system::Config>::RuntimeCall = inner.into();

		// The message includes the inherited_implication, "revise", account, and nonce.
		// In test_run, inherited_implication comes from (ext_version, &call).
		let ext_version: ExtensionVersion = 0;
		let msg = ((ext_version, &call), "revise", &caller, nonce)
			.using_encoded(sp_io::hashing::blake2_256);

		let ring_members = T::MemberService::ring_members(PEOPLE_MEMBER_IDENTIFIER, 0);
		let capacity: <CryptoOf<T> as GenerateVerifiable>::Config = T::RingExponent::get()
			.try_into()
			.ok()
			.expect("ring exponent must convert to capacity");
		let commitment = CryptoOf::<T>::open(capacity, public, ring_members.into_iter())
			.expect("open must succeed");
		let (proof, _alias) = CryptoOf::<T>::create(commitment, secret, &context[..], &msg)
			.expect("create must succeed");

		let ext = AsPerson::new(Some(AsPersonInfo::<T>::AsPersonalAliasWithAccountRevised(
			nonce, proof, 0, context,
		)));
		let info = call.get_dispatch_info();
		let post_info = PostDispatchInfo {
			actual_weight: Some(Weight::from_parts(10, 0)),
			pays_fee: Pays::Yes,
		};
		let len = call.encoded_size();

		#[block]
		{
			ext.test_run(RawOrigin::Signed(caller.clone()).into(), &call, &info, len, 0, |_| {
				Ok(post_info)
			})
			.unwrap()
			.unwrap();
		}

		assert_eq!(
			AccountToAlias::<T>::get(&caller).expect("binding must exist").revision,
			new_revision,
		);
		assert_eq!(frame_system::Pallet::<T>::account_nonce(&caller), 1u32.into());

		Ok(())
	}

	#[benchmark]
	fn create_people_collection() -> Result<(), BenchmarkError> {
		#[extrinsic_call]
		_(frame_system::Origin::<T>::Authorized);

		// Verify the collection was created
		assert!(PeopleCollectionCreated::<T>::get());

		frame_system::Pallet::<T>::assert_last_event(crate::Event::<T>::CollectionCreated.into());

		Ok(())
	}

	#[benchmark]
	fn authorize_create_people_collection() -> Result<(), BenchmarkError> {
		#[block]
		{
			Pallet::<T>::authorize_create_people_collection(TransactionSource::InBlock)
				.expect("authorization must succeed when collection is not yet created");
		}

		Ok(())
	}

	/// Benchmark for cleaning up stale alias mappings.
	///
	/// Worst case: all n aliases exist, are correctly mapped, and are stale because
	/// their ring has been deleted.
	#[benchmark]
	fn clean_up_stale_alias(
		n: Linear<1, { pallet::MAX_BULK_CLEANUP }>,
	) -> Result<(), BenchmarkError> {
		let context = T::BenchmarkHelper::worst_case_account_context();
		let stale_ring: u32 = 0;

		let mut aliases = Vec::with_capacity(n as usize);
		let mut accounts = Vec::with_capacity(n as usize);

		for i in 0..n {
			let acc: T::AccountId = account("stale", i, SEED);
			let mut alias_value: Alias = [0u8; 32];
			alias_value[..4].copy_from_slice(&i.to_le_bytes());

			let ca = ContextualAlias { context, alias: alias_value };
			let rev_ca = RevisedContextualAlias { ca: ca.clone(), revision: 0, ring: stale_ring };

			// Direct mutate: reaching the stale state via real APIs would require
			// building a ring, binding `n` aliases with proofs, then deleting it —
			// costly setup that doesn't affect what we measure.
			AliasToAccount::<T>::insert(&ca, &acc);
			AccountToAlias::<T>::insert(&acc, &rev_ca);
			frame_system::Pallet::<T>::inc_sufficients(&acc);

			aliases.push(ca);
			accounts.push(acc);
		}

		let bounded_aliases: BoundedVec<ContextualAlias, ConstU32<{ pallet::MAX_BULK_CLEANUP }>> =
			BoundedVec::try_from(aliases.clone()).expect("n <= MAX_BULK_CLEANUP");

		#[extrinsic_call]
		clean_up_stale_aliases(frame_system::Origin::<T>::Authorized, bounded_aliases);

		for (ca, acc) in aliases.iter().zip(accounts.iter()) {
			assert!(!AliasToAccount::<T>::contains_key(ca));
			assert!(!AccountToAlias::<T>::contains_key(acc));
		}

		frame_system::Pallet::<T>::assert_last_event(
			crate::Event::<T>::AliasCleanedUp {
				alias: aliases.last().unwrap().clone(),
				account: accounts.last().unwrap().clone(),
			}
			.into(),
		);

		Ok(())
	}

	/// Benchmark for authorizing stale alias cleanup.
	///
	/// Worst case: all n aliases exist, are correctly mapped, and are stale because
	/// their ring has been deleted. Forces `ensure_alias_is_stale` to perform BOTH
	/// storage reads instead of short-circuiting on a missing context.
	#[benchmark]
	fn authorize_clean_up_stale_alias(
		n: Linear<1, { pallet::MAX_BULK_CLEANUP }>,
	) -> Result<(), BenchmarkError> {
		let context = T::BenchmarkHelper::worst_case_account_context();
		let stale_ring: u32 = 0;

		let mut aliases = Vec::with_capacity(n as usize);

		for i in 0..n {
			let account: T::AccountId = account("stale_auth", i, SEED);
			let mut alias_value: Alias = [0u8; 32];
			alias_value[..4].copy_from_slice(&i.to_le_bytes());

			let ca = ContextualAlias { context, alias: alias_value };
			let rev_ca = RevisedContextualAlias { ca: ca.clone(), revision: 0, ring: stale_ring };

			AliasToAccount::<T>::insert(&ca, &account);
			AccountToAlias::<T>::insert(&account, &rev_ca);
			frame_system::Pallet::<T>::inc_sufficients(&account);

			aliases.push(ca);
		}

		let bounded_aliases: BoundedVec<ContextualAlias, ConstU32<{ pallet::MAX_BULK_CLEANUP }>> =
			BoundedVec::try_from(aliases).expect("n <= MAX_BULK_CLEANUP");

		let call = Call::<T>::clean_up_stale_aliases { aliases: bounded_aliases };

		#[block]
		{
			call.authorize(TransactionSource::InBlock)
				.ok_or("Call must give some authorization")??;
		}

		Ok(())
	}

	// Implements a test for each benchmark. Execute with:
	// `cargo test -p pallet-people --features runtime-benchmarks`.
	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
