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

//! Resources pallet benchmarks

#![allow(unused)]

use super::*;
use crate::Pallet as Resources;
use codec::{Decode, Encode};
use core::time::Duration;
use frame_benchmarking::v2::{benchmarks, *};
use frame_support::{
	assert_ok,
	dispatch::{DispatchInfo, PostDispatchInfo},
	pallet_prelude::Authorize,
	traits::{EnsureOrigin, IsSubType},
	BoundedVec,
};
use frame_system::{pallet_prelude::BlockNumberFor, RawOrigin as SystemOrigin};
use indiv_support::traits::{AppendOnlyMembers, MembershipProver, RingMode};
use sp_core::Get;
use sp_runtime::{
	traits::{
		AsTransactionAuthorizedOrigin, DispatchTransaction, Dispatchable, Saturating,
		TxBaseImplication, Verify, Zero,
	},
	transaction_validity::TransactionSource,
};
use sp_statement_store::{
	decrease_allowance_by, get_allowance, increase_allowance_by, StatementAllowance,
};
use verifiable::GenerateVerifiable;

pub type CryptoOf<T> = <<T as Config>::MemberService as MembershipProver>::Crypto;
pub type MemberOf<T> = <CryptoOf<T> as GenerateVerifiable>::Member;
pub type SecretOf<T> = <CryptoOf<T> as GenerateVerifiable>::Secret;

/// Benchmark helper trait.
pub trait BenchmarkHelper<T: Config> {
	/// Sets a time in seconds since the UNIX epoch for benchmarks.
	fn set_time(now: Duration);
	/// Sign a message.
	fn sign_message(message: &[u8]) -> (T::AccountId, T::OffchainSignature);
}

/// Stable inputs used to benchmark a runtime's account-bound Meta policy router.
///
/// The router is runtime-owned (its call and signed-extension types are runtime specific), so the
/// Resources benchmark target deliberately transports only this bounded scenario tag across the
/// pallet/runtime boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetaPolicyBenchmarkScenario {
	PersonalAlias,
	PersonalIdentity,
	PersonalAliasRevised,
	LitePerson,
	LiteAlias,
	LiteAliasRevised,
	ResourcesClaim,
	Malformed,
	MappingMiss,
	RevisedWrite,
	MaxProof,
	Envelope,
}

/// Runtime hook for the Meta policy benchmarks hosted by this pallet.
pub trait MetaPolicyBenchmarkHelper {
	fn run(scenario: MetaPolicyBenchmarkScenario) -> Result<(), BenchmarkError>;
}

/// A no-op fallback keeps generic/mocked Resources runtimes source compatible. Production
/// runtimes that expose benchmark metadata must replace it with a helper that invokes their
/// actual router.
impl MetaPolicyBenchmarkHelper for () {
	fn run(_: MetaPolicyBenchmarkScenario) -> Result<(), BenchmarkError> {
		Ok(())
	}
}

// --- Helpers

fn assert_last_event<T: Config>(generic_event: <T as frame_system::Config>::RuntimeEvent) {
	frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

fn setup_people_ring_with_one_member<
	T: Config + indiv_pallet_people::Config<MemberService = <T as Config>::MemberService>,
>() -> Result<(SecretOf<T>, MemberOf<T>), BenchmarkError> {
	let owner = Decode::decode(&mut &[0u8; 32][..])
		.map_err(|_| BenchmarkError::Stop("failed to decode collection owner"))?;
	<T as Config>::MemberService::create_collection(
		owner,
		indiv_pallet_people::PEOPLE_MEMBER_IDENTIFIER,
		1,
		RingMode::Flexible,
		<T as indiv_pallet_people::Config>::RingExponent::get(),
		None,
	)
	.map_err(|_| BenchmarkError::Stop("failed to create people collection"))?;

	let secret = CryptoOf::<T>::new_secret([0u8; 32]);
	let member = CryptoOf::<T>::member_from_secret(&secret);
	<T as Config>::MemberService::add_members(
		indiv_pallet_people::PEOPLE_MEMBER_IDENTIFIER,
		alloc::vec![member.clone()],
	)
	.map_err(|_| BenchmarkError::Stop("failed to add people member"))?;

	<T as Config>::MemberService::initialize_chunks(
		<T as indiv_pallet_people::Config>::RingExponent::get(),
	);
	<T as Config>::MemberService::onboard_all_and_build_ring(
		indiv_pallet_people::PEOPLE_MEMBER_IDENTIFIER,
		0,
	)
	.map_err(|_| BenchmarkError::Stop("failed to build people ring"))?;

	Ok((secret, member))
}

fn setup_lite_ring_with_one_member<
	T: Config + indiv_pallet_people_lite::Config<MemberService = <T as Config>::MemberService>,
>() -> Result<(SecretOf<T>, MemberOf<T>), BenchmarkError> {
	<T as Config>::MemberService::create_collection(
		<T as indiv_pallet_people_lite::Config>::CollectionOwner::get(),
		indiv_pallet_people_lite::LITE_PEOPLE_MEMBER_IDENTIFIER,
		<T as indiv_pallet_people_lite::Config>::LiteOnboardingSize::get().max(1),
		RingMode::AppendOnly,
		<T as indiv_pallet_people_lite::Config>::LiteRingExponent::get(),
		None,
	)
	.map_err(|_| BenchmarkError::Stop("failed to create lite people collection"))?;

	let secret = CryptoOf::<T>::new_secret([1u8; 32]);
	let member = CryptoOf::<T>::member_from_secret(&secret);
	let cohort_size = <T as indiv_pallet_people_lite::Config>::LiteOnboardingSize::get().max(1);
	let mut members = alloc::vec![member.clone()];
	for i in 1..cohort_size {
		let entropy = (b"lite-benchmark-member", i).using_encoded(sp_io::hashing::blake2_256);
		let extra_secret = CryptoOf::<T>::new_secret(entropy);
		members.push(CryptoOf::<T>::member_from_secret(&extra_secret));
	}

	<T as Config>::MemberService::add_members(
		indiv_pallet_people_lite::LITE_PEOPLE_MEMBER_IDENTIFIER,
		members,
	)
	.map_err(|_| BenchmarkError::Stop("failed to add lite people members"))?;

	<T as Config>::MemberService::initialize_chunks(
		<T as indiv_pallet_people_lite::Config>::LiteRingExponent::get(),
	);
	<T as Config>::MemberService::onboard_all_and_build_ring(
		indiv_pallet_people_lite::LITE_PEOPLE_MEMBER_IDENTIFIER,
		0,
	)
	.map_err(|_| BenchmarkError::Stop("failed to build lite people ring"))?;

	Ok((secret, member))
}

#[benchmarks(
	where T:
		// for benchmarks we directly depend on people and people-lite to avoid having to require
		// benchmark helpers to set up collections.
		indiv_pallet_people::Config<
			MemberService = <T as Config>::MemberService,
		>
		+ indiv_pallet_people_lite::Config<
			MemberService = <T as Config>::MemberService,
		>,
		<T as frame_system::Config>::RuntimeCall: IsSubType<Call<T>> + From<Call<T>>,
		<T as frame_system::Config>::RuntimeOrigin: AsTransactionAuthorizedOrigin,
)]
mod benches {
	use super::*;

	#[benchmark]
	fn meta_policy_personal_alias() -> Result<(), BenchmarkError> {
		#[block]
		{
			<T as Config>::MetaPolicyBenchmarkHelper::run(
				MetaPolicyBenchmarkScenario::PersonalAlias,
			)?;
		}
		Ok(())
	}

	#[benchmark]
	fn meta_policy_personal_identity() -> Result<(), BenchmarkError> {
		#[block]
		{
			<T as Config>::MetaPolicyBenchmarkHelper::run(
				MetaPolicyBenchmarkScenario::PersonalIdentity,
			)?;
		}
		Ok(())
	}

	#[benchmark]
	fn meta_policy_personal_alias_revised() -> Result<(), BenchmarkError> {
		#[block]
		{
			<T as Config>::MetaPolicyBenchmarkHelper::run(
				MetaPolicyBenchmarkScenario::PersonalAliasRevised,
			)?;
		}
		Ok(())
	}

	#[benchmark]
	fn meta_policy_lite_person() -> Result<(), BenchmarkError> {
		#[block]
		{
			<T as Config>::MetaPolicyBenchmarkHelper::run(MetaPolicyBenchmarkScenario::LitePerson)?;
		}
		Ok(())
	}

	#[benchmark]
	fn meta_policy_lite_alias() -> Result<(), BenchmarkError> {
		#[block]
		{
			<T as Config>::MetaPolicyBenchmarkHelper::run(MetaPolicyBenchmarkScenario::LiteAlias)?;
		}
		Ok(())
	}

	#[benchmark]
	fn meta_policy_lite_alias_revised() -> Result<(), BenchmarkError> {
		#[block]
		{
			<T as Config>::MetaPolicyBenchmarkHelper::run(
				MetaPolicyBenchmarkScenario::LiteAliasRevised,
			)?;
		}
		Ok(())
	}

	#[benchmark]
	fn meta_policy_resources_claim() -> Result<(), BenchmarkError> {
		#[block]
		{
			<T as Config>::MetaPolicyBenchmarkHelper::run(
				MetaPolicyBenchmarkScenario::ResourcesClaim,
			)?;
		}
		Ok(())
	}

	#[benchmark]
	fn meta_policy_malformed() -> Result<(), BenchmarkError> {
		#[block]
		{
			<T as Config>::MetaPolicyBenchmarkHelper::run(MetaPolicyBenchmarkScenario::Malformed)?;
		}
		Ok(())
	}

	#[benchmark]
	fn meta_policy_mapping_miss() -> Result<(), BenchmarkError> {
		#[block]
		{
			<T as Config>::MetaPolicyBenchmarkHelper::run(
				MetaPolicyBenchmarkScenario::MappingMiss,
			)?;
		}
		Ok(())
	}

	#[benchmark]
	fn meta_policy_revised_write() -> Result<(), BenchmarkError> {
		#[block]
		{
			<T as Config>::MetaPolicyBenchmarkHelper::run(
				MetaPolicyBenchmarkScenario::RevisedWrite,
			)?;
		}
		Ok(())
	}

	#[benchmark]
	fn meta_policy_max_proof() -> Result<(), BenchmarkError> {
		#[block]
		{
			<T as Config>::MetaPolicyBenchmarkHelper::run(MetaPolicyBenchmarkScenario::MaxProof)?;
		}
		Ok(())
	}

	#[benchmark]
	fn meta_policy_envelope() -> Result<(), BenchmarkError> {
		#[block]
		{
			<T as Config>::MetaPolicyBenchmarkHelper::run(MetaPolicyBenchmarkScenario::Envelope)?;
		}
		Ok(())
	}

	#[benchmark]
	fn register_lite_person() -> Result<(), BenchmarkError> {
		let origin =
			T::EnsureLitePerson::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;
		let Ok(lite_account) = T::EnsureLitePerson::try_origin(origin.clone()) else {
			panic!("origin was created with `try_successful_origin`; qed");
		};
		let identifier_key = [0u8; 65];
		let username = Username::try_from(b"validusername.12".to_vec()).unwrap();
		let reserved = Username::try_from(b"validreserved".to_vec()).unwrap();
		<T as Config>::BenchmarkHelper::set_time(Duration::from_secs(1000));

		let max_len = T::MaxReservationQueueLength::get();
		let prefill = max_len.saturating_sub(1);
		let queue: BoundedVec<ReservationQueueEntryOf<T>, T::MaxReservationQueueLength> = (0..
			prefill)
			.map(|i| {
				let acc: T::AccountId = account("queue", i, 0);
				ReservationOf::<T>::insert(&acc, &reserved);
				ReservationQueueEntryOf::<T> { account: acc, joined_at: 1000 }
			})
			.collect::<alloc::vec::Vec<_>>()
			.try_into()
			.unwrap();
		UsernameReservationQueue::<T>::insert(&reserved, queue);

		#[extrinsic_call]
		_(origin.clone(), identifier_key, username, Some(reserved.clone()));

		assert_last_event::<T>(Event::LitePersonRegistered { account: lite_account }.into());
		let queue = UsernameReservationQueue::<T>::get(&reserved).expect("queue exists");
		assert_eq!(queue.len() as u32, max_len);
		Ok(())
	}

	#[benchmark]
	fn register_person_standalone() -> Result<(), BenchmarkError> {
		let identifier_key = [0u8; 65];
		let lite_uname = Username::try_from(b"validusername.12".to_vec()).unwrap();
		let person_uname = Username::try_from(b"personuser".to_vec()).unwrap();
		<T as Config>::BenchmarkHelper::set_time(Duration::from_secs(1000));

		let (lite_account, _) = <T as Config>::BenchmarkHelper::sign_message(b"mock");
		UsernameOwnerOf::<T>::insert(&lite_uname, &lite_account);
		let info = ConsumerInfo {
			identifier_key,
			full_username: None,
			lite_username: lite_uname,
			credibility: Credibility::Lite,
		};
		Consumers::<T>::insert(&lite_account, info);
		frame_system::Pallet::<T>::inc_sufficients(&lite_account);

		let n = T::MaxReservationQueueLength::get();
		let other_reserved = Username::try_from(b"otherreserved".to_vec()).unwrap();
		let mut entries = alloc::vec::Vec::new();
		entries
			.push(ReservationQueueEntryOf::<T> { account: lite_account.clone(), joined_at: 1000 });
		for i in 0..(n - 1) {
			let acc: T::AccountId = account("queue", i, 0);
			ReservationOf::<T>::insert(&acc, &other_reserved);
			entries.push(ReservationQueueEntryOf::<T> { account: acc, joined_at: 1000 });
		}
		let queue: BoundedVec<ReservationQueueEntryOf<T>, T::MaxReservationQueueLength> =
			BoundedVec::try_from(entries).unwrap();
		UsernameReservationQueue::<T>::insert(&other_reserved, queue);
		ReservationOf::<T>::insert(&lite_account, &other_reserved);

		let origin = T::EnsurePerson::try_successful_origin(&RESOURCES_CONTEXT)
			.map_err(|_| BenchmarkError::Weightless)?;
		let Ok(alias) = T::EnsurePerson::try_origin(origin.clone(), &RESOURCES_CONTEXT) else {
			panic!("origin was created with `try_successful_origin`; qed");
		};
		let (_, proof) = <T as Config>::BenchmarkHelper::sign_message(&alias[..]);

		#[extrinsic_call]
		register_person(
			origin as T::RuntimeOrigin,
			lite_account.clone(),
			proof,
			PersonalUsernameChoice::Standalone(person_uname),
		);

		assert_last_event::<T>(Event::PersonRegistered { account: lite_account, alias }.into());
		Ok(())
	}

	#[benchmark]
	fn register_person_reservation() -> Result<(), BenchmarkError> {
		let identifier_key = [0u8; 65];
		let lite_uname = Username::try_from(b"validusername.12".to_vec()).unwrap();
		let reserved = Username::try_from(b"validreserved".to_vec()).unwrap();
		<T as Config>::BenchmarkHelper::set_time(Duration::from_secs(1000));

		let (lite_account, _) = <T as Config>::BenchmarkHelper::sign_message(b"mock");
		UsernameOwnerOf::<T>::insert(&lite_uname, &lite_account);
		let info = ConsumerInfo {
			identifier_key,
			full_username: None,
			lite_username: lite_uname,
			credibility: Credibility::Lite,
		};
		Consumers::<T>::insert(&lite_account, info);
		frame_system::Pallet::<T>::inc_sufficients(&lite_account);

		let n = T::MaxReservationQueueLength::get();
		let mut entries = alloc::vec::Vec::new();
		entries
			.push(ReservationQueueEntryOf::<T> { account: lite_account.clone(), joined_at: 1000 });
		ReservationOf::<T>::insert(&lite_account, &reserved);
		for i in 1..n {
			let acc: T::AccountId = account("queue", i, 0);
			ReservationOf::<T>::insert(&acc, &reserved);
			entries.push(ReservationQueueEntryOf::<T> { account: acc, joined_at: 1000 });
		}
		let queue: BoundedVec<ReservationQueueEntryOf<T>, T::MaxReservationQueueLength> =
			BoundedVec::try_from(entries).unwrap();
		UsernameReservationQueue::<T>::insert(&reserved, queue);

		let origin = T::EnsurePerson::try_successful_origin(&RESOURCES_CONTEXT)
			.map_err(|_| BenchmarkError::Weightless)?;
		let Ok(alias) = T::EnsurePerson::try_origin(origin.clone(), &RESOURCES_CONTEXT) else {
			panic!("origin was created with `try_successful_origin`; qed");
		};
		let (_, proof) = <T as Config>::BenchmarkHelper::sign_message(&alias[..]);

		#[extrinsic_call]
		register_person(
			origin as T::RuntimeOrigin,
			lite_account.clone(),
			proof,
			PersonalUsernameChoice::Reservation(reserved),
		);

		assert_last_event::<T>(Event::PersonRegistered { account: lite_account, alias }.into());
		Ok(())
	}

	#[benchmark]
	fn touch_person_authorization() -> Result<(), BenchmarkError> {
		let identifier_key = [0u8; 65];
		let username = Username::try_from(b"validusername.12".to_vec()).unwrap();
		let reserved = Username::try_from(b"validreserved".to_vec()).unwrap();
		<T as Config>::BenchmarkHelper::set_time(Duration::from_secs(1000));

		let (account, _) = <T as Config>::BenchmarkHelper::sign_message(b"mock");
		let queue_entry = ReservationQueueEntryOf::<T> {
			account: account.clone(),
			joined_at: T::Clock::now().as_secs(),
		};
		let queue: BoundedVec<ReservationQueueEntryOf<T>, T::MaxReservationQueueLength> =
			BoundedVec::try_from(alloc::vec![queue_entry]).unwrap();
		UsernameReservationQueue::<T>::insert(&reserved, queue);
		ReservationOf::<T>::insert(&account, &reserved);
		UsernameOwnerOf::<T>::insert(&username, &account);

		let origin = T::EnsurePerson::try_successful_origin(&RESOURCES_CONTEXT)
			.map_err(|_| BenchmarkError::Weightless)?;
		let Ok(alias) = T::EnsurePerson::try_origin(origin.clone(), &RESOURCES_CONTEXT) else {
			panic!("origin was created with `try_successful_origin`; qed");
		};

		let info = ConsumerInfo {
			identifier_key,
			full_username: None,
			lite_username: username,
			credibility: Credibility::Lite,
		};
		Consumers::<T>::insert(&account, info);
		frame_system::Pallet::<T>::inc_sufficients(&account);

		let (_, proof) = <T as Config>::BenchmarkHelper::sign_message(&alias[..]);
		assert_ok!(Pallet::<T>::register_person(
			origin.clone(),
			account.clone(),
			proof,
			PersonalUsernameChoice::Reservation(reserved),
		));

		<T as Config>::BenchmarkHelper::set_time(Duration::from_secs(
			1000 + T::PersonAuthDuration::get() as u64 + 1,
		));

		// Demote the person so we benchmark the worst case (was_demoted path).
		assert_ok!(Pallet::<T>::demote_auth_expired(
			SystemOrigin::Authorized.into(),
			account.clone()
		));

		match Consumers::<T>::get(&account)
			.expect("account has just been included")
			.credibility
		{
			Credibility::Lite => panic!("expected Person credibility"),
			Credibility::Person { demoted, .. } => assert!(demoted),
		}

		#[extrinsic_call]
		_(origin);

		let now = T::Clock::now().as_secs();
		assert!(matches!(
			Consumers::<T>::get(&account).unwrap().credibility,
			Credibility::Person { last_update, demoted: false, .. } if last_update == now
		));

		Ok(())
	}

	#[benchmark]
	fn validate_reservation_expiry() -> Result<(), BenchmarkError> {
		let reserved = Username::try_from(b"validreserved".to_vec()).unwrap();
		let max_len = T::MaxReservationQueueLength::get() as u32;
		UsernameReservationDuration::<T>::put(60);

		// Fill the queue to capacity. The target account is the last entry so the
		// linear scan in `validate_reservation_expiry` hits the worst case.
		let mut entries = alloc::vec::Vec::new();
		for i in 0..max_len {
			let acc: T::AccountId = account("queue", i, 0);
			ReservationOf::<T>::insert(&acc, &reserved);
			entries.push(ReservationQueueEntryOf::<T> { account: acc, joined_at: 1000 });
		}
		let queue: BoundedVec<ReservationQueueEntryOf<T>, T::MaxReservationQueueLength> =
			BoundedVec::try_from(entries).unwrap();
		let target = queue.last().unwrap().account.clone();
		UsernameReservationQueue::<T>::insert(&reserved, queue);

		// Advance time past expiry.
		<T as Config>::BenchmarkHelper::set_time(Duration::from_secs(1000 + 60 + 1));

		#[block]
		{
			Resources::<T>::validate_reservation_expiry(&reserved, &target).unwrap();
		}

		Ok(())
	}

	#[benchmark]
	fn remove_expired_username_reservation() -> Result<(), BenchmarkError> {
		let reserved = Username::try_from(b"validreserved".to_vec()).unwrap();
		let max_len = T::MaxReservationQueueLength::get() as u32;
		UsernameReservationDuration::<T>::put(60);

		// Fill the queue to capacity. The target account is at the front so that
		// `Vec::remove(pos=0)` performs the maximum (n-1) shifts inside
		// `remove_username_reservation`
		let mut entries = alloc::vec::Vec::new();
		for i in 0..max_len {
			let acc: T::AccountId = account("queue", i, 0);
			entries.push(ReservationQueueEntryOf::<T> { account: acc.clone(), joined_at: 1000 });
			ReservationOf::<T>::insert(&acc, &reserved);
		}
		let queue: BoundedVec<ReservationQueueEntryOf<T>, T::MaxReservationQueueLength> =
			BoundedVec::try_from(entries).unwrap();
		let target = queue.first().unwrap().account.clone();
		UsernameReservationQueue::<T>::insert(&reserved, queue);

		// Advance time past expiry.
		<T as Config>::BenchmarkHelper::set_time(Duration::from_secs(1000 + 60 + 1));

		#[extrinsic_call]
		_(SystemOrigin::Authorized, reserved.clone(), target.clone());

		assert_eq!(ReservationOf::<T>::get(&target), None);
		let remaining = UsernameReservationQueue::<T>::get(&reserved).unwrap();
		assert_eq!(remaining.len() as u32, max_len - 1);

		Ok(())
	}

	#[benchmark]
	fn update_identifier_key() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let lite_username = Username::try_from([b'a'; 32].to_vec()).unwrap();
		let full_username = Username::try_from([b'b'; 32].to_vec()).unwrap();
		let alias: Alias = [1u8; 32];
		let consumer_info = ConsumerInfo {
			identifier_key: [0u8; 65],
			full_username: Some(full_username),
			lite_username,
			credibility: Credibility::Person { alias, last_update: 0, demoted: false },
		};
		Consumers::<T>::insert(&caller, consumer_info);

		let new_key = [1u8; 65];

		#[extrinsic_call]
		_(SystemOrigin::Signed(caller.clone()), new_key);

		assert_eq!(Consumers::<T>::get(caller).unwrap().identifier_key, new_key);

		Ok(())
	}

	#[benchmark]
	fn set_username_reservation_duration() -> Result<(), BenchmarkError> {
		let new_duration = 12345u64;

		let origin = <T as Config>::ManagerOrigin::try_successful_origin()
			.map_err(|_| BenchmarkError::Weightless)?;

		#[extrinsic_call]
		_(origin as T::RuntimeOrigin, new_duration);

		assert_eq!(UsernameReservationDuration::<T>::get(), new_duration);

		Ok(())
	}

	#[benchmark]
	fn demote_auth_expired() -> Result<(), BenchmarkError> {
		let account: T::AccountId = whitelisted_caller();
		let lite_username = Username::try_from([b'a'; 32].to_vec()).unwrap();
		let full_username = Username::try_from([b'b'; 32].to_vec()).unwrap();
		let alias: Alias = [1u8; 32];

		let pre_allowance = T::PersonStatementLimit::get();
		let post_allowance = T::LitePersonStatementLimit::get();
		increase_allowance_by(account.clone().into(), pre_allowance);

		let consumer_info = ConsumerInfo {
			identifier_key: [0u8; 65],
			full_username: Some(full_username),
			lite_username,
			credibility: Credibility::Person { alias, last_update: 0, demoted: false },
		};
		let mut post_consumer_info = consumer_info.clone();
		post_consumer_info.credibility =
			Credibility::Person { alias, last_update: 0, demoted: true };
		Consumers::<T>::insert(&account, consumer_info);
		<T as Config>::BenchmarkHelper::set_time(Duration::from_secs(
			T::PersonAuthDuration::get() as u64 + 1,
		));

		#[extrinsic_call]
		_(SystemOrigin::Authorized, account.clone());

		assert_eq!(Consumers::<T>::get(&account).unwrap(), post_consumer_info);
		assert_eq!(get_allowance(account.into()), post_allowance);
		Ok(())
	}

	#[benchmark]
	fn authorize_demote_auth_expired() -> Result<(), BenchmarkError> {
		let account: T::AccountId = whitelisted_caller();
		let lite_username = Username::try_from([b'a'; 32].to_vec()).unwrap();
		let full_username = Username::try_from([b'b'; 32].to_vec()).unwrap();
		let alias: Alias = [1u8; 32];

		Consumers::<T>::insert(
			&account,
			ConsumerInfo {
				identifier_key: [0u8; 65],
				full_username: Some(full_username),
				lite_username,
				credibility: Credibility::Person { alias, last_update: 0, demoted: false },
			},
		);
		<T as Config>::BenchmarkHelper::set_time(Duration::from_secs(
			T::PersonAuthDuration::get() as u64 + 1,
		));

		let call = Call::<T>::demote_auth_expired { account };

		#[block]
		{
			call.authorize(TransactionSource::InBlock)
				.ok_or("Call must give some authorization")??;
		}

		Ok(())
	}

	#[benchmark]
	fn set_friend_request_statement_account_for_sequence() -> Result<(), BenchmarkError> {
		let period_duration = T::FriendRequestPeriodDuration::get().max(1) as u64;
		<T as Config>::BenchmarkHelper::set_time(Duration::from_secs(period_duration + 1));

		let period = Pallet::<T>::friend_request_period_from_timestamp(T::Clock::now().as_secs());
		let reference = crate::types::FriendRequestReference { period, seq: 0 };
		let alias: Alias = [1u8; 32];
		let origin = <T as frame_system::Config>::RuntimeOrigin::from(
			crate::Origin::FriendRequestAlias(alias),
		);
		let account: T::AccountId = whitelisted_caller();

		#[extrinsic_call]
		_(origin, reference, account.clone());

		let registration = FriendRequestRegistrationByAlias::<T>::get(alias)
			.expect("registration should be stored");
		assert_eq!(registration.account_id, account.clone());
		assert_eq!(registration.reference, reference);
		assert_eq!(FriendRequestAliasByAccount::<T>::get(&account), Some(alias));
		Ok(())
	}

	#[benchmark]
	fn clear_expired_friend_request_sequence() -> Result<(), BenchmarkError> {
		let period_duration = T::FriendRequestPeriodDuration::get().max(1) as u64;
		<T as Config>::BenchmarkHelper::set_time(Duration::from_secs(period_duration + 1));

		let period = Pallet::<T>::friend_request_period_from_timestamp(T::Clock::now().as_secs());
		let reference = crate::types::FriendRequestReference { period, seq: 0 };
		let alias: Alias = [2u8; 32];
		let origin = <T as frame_system::Config>::RuntimeOrigin::from(
			crate::Origin::FriendRequestAlias(alias),
		);
		let account: T::AccountId = whitelisted_caller();

		assert_ok!(Pallet::<T>::set_friend_request_statement_account_for_sequence(
			origin,
			reference,
			account.clone(),
		));

		let cleanup_time = Pallet::<T>::friend_request_expiration_time(period).saturating_add(1);
		<T as Config>::BenchmarkHelper::set_time(Duration::from_secs(cleanup_time));

		#[extrinsic_call]
		_(SystemOrigin::Authorized, account.clone(), reference.seq);

		assert_eq!(FriendRequestAliasByAccount::<T>::get(&account), None);
		assert_eq!(FriendRequestRegistrationByAlias::<T>::get(alias), None);
		Ok(())
	}

	#[benchmark]
	fn authorize_clear_expired_friend_request_sequence() -> Result<(), BenchmarkError> {
		let period_duration = T::FriendRequestPeriodDuration::get().max(1) as u64;
		<T as Config>::BenchmarkHelper::set_time(Duration::from_secs(period_duration + 1));

		let period = Pallet::<T>::friend_request_period_from_timestamp(T::Clock::now().as_secs());
		let reference = crate::types::FriendRequestReference { period, seq: 0 };
		let alias: Alias = [3u8; 32];
		let origin = <T as frame_system::Config>::RuntimeOrigin::from(
			crate::Origin::FriendRequestAlias(alias),
		);
		let account: T::AccountId = whitelisted_caller();

		assert_ok!(Pallet::<T>::set_friend_request_statement_account_for_sequence(
			origin,
			reference,
			account.clone(),
		));

		let cleanup_time = Pallet::<T>::friend_request_expiration_time(period).saturating_add(1);
		<T as Config>::BenchmarkHelper::set_time(Duration::from_secs(cleanup_time));

		let call = Call::<T>::clear_expired_friend_request_sequence { account, seq: reference.seq };

		#[block]
		{
			call.authorize(TransactionSource::InBlock)
				.ok_or("Call must give some authorization")??;
		}

		Ok(())
	}

	#[benchmark]
	fn as_register_with_proof_tx_ext() -> Result<(), BenchmarkError> {
		let now = Duration::from_secs(1000);
		<T as Config>::BenchmarkHelper::set_time(now);

		let (secret, member) = setup_people_ring_with_one_member::<T>()?;
		let reference = crate::types::FriendRequestReference {
			period: Resources::<T>::friend_request_period_from_timestamp(now.as_secs()),
			seq: 0,
		};
		let call = Call::<T>::set_friend_request_statement_account_for_sequence {
			reference,
			account_id: whitelisted_caller(),
		};
		let call: <T as frame_system::Config>::RuntimeCall = call.into();
		let extension_version = 0u8;

		let msg =
			TxBaseImplication((extension_version, &call)).using_encoded(sp_io::hashing::blake2_256);
		let context = Resources::<T>::friend_request_context(reference);
		let ring_members = <T as indiv_pallet_people::Config>::MemberService::ring_members(
			indiv_pallet_people::PEOPLE_MEMBER_IDENTIFIER,
			0,
		);
		let capacity: <indiv_pallet_people::CryptoOf<T> as GenerateVerifiable>::Config =
			<T as indiv_pallet_people::Config>::RingExponent::get()
				.try_into()
				.map_err(|_| BenchmarkError::Stop("invalid ring exponent"))?;
		let commitment =
			indiv_pallet_people::CryptoOf::<T>::open(capacity, &member, ring_members.into_iter())
				.map_err(|_| BenchmarkError::Stop("failed to open commitment"))?;
		let (proof, _) =
			indiv_pallet_people::CryptoOf::<T>::create(commitment, &secret, &context, &msg)
				.map_err(|_| BenchmarkError::Stop("failed to create proof"))?;

		let tx_ext = crate::extension::AsResources::<T>::new(Some(
			crate::extension::AsResourcesInfo::RegisterFriendRequestWithProof(proof, 0),
		));
		let len = call.encode().len();

		#[block]
		{
			tx_ext
				.test_run(SystemOrigin::None.into(), &call, &Default::default(), len, 0, |_| {
					Ok(Default::default())
				})
				.unwrap()?;
		}

		Ok(())
	}

	#[benchmark]
	fn as_register_for_collection_tx_ext() -> Result<(), BenchmarkError> {
		let now = Duration::from_secs(1000);
		<T as Config>::BenchmarkHelper::set_time(now);

		let (secret, member) = setup_lite_ring_with_one_member::<T>()?;
		let reference = crate::types::FriendRequestReference {
			period: Resources::<T>::friend_request_period_from_timestamp(now.as_secs()),
			seq: 0,
		};
		let call = Call::<T>::set_friend_request_statement_account_for_sequence {
			reference,
			account_id: whitelisted_caller(),
		};
		let call: <T as frame_system::Config>::RuntimeCall = call.into();
		let extension_version = 0u8;

		let msg =
			TxBaseImplication((extension_version, &call)).using_encoded(sp_io::hashing::blake2_256);
		let context = Resources::<T>::friend_request_context(reference);
		let ring_members = <T as indiv_pallet_people_lite::Config>::MemberService::ring_members(
			indiv_pallet_people_lite::LITE_PEOPLE_MEMBER_IDENTIFIER,
			0,
		);
		let capacity: <indiv_pallet_people::CryptoOf<T> as GenerateVerifiable>::Config =
			<T as indiv_pallet_people_lite::Config>::LiteRingExponent::get()
				.try_into()
				.map_err(|_| BenchmarkError::Stop("invalid lite ring exponent"))?;
		let commitment =
			indiv_pallet_people::CryptoOf::<T>::open(capacity, &member, ring_members.into_iter())
				.map_err(|_| BenchmarkError::Stop("failed to open lite commitment"))?;
		let (proof, _) =
			indiv_pallet_people::CryptoOf::<T>::create(commitment, &secret, &context, &msg)
				.map_err(|_| BenchmarkError::Stop("failed to create lite proof"))?;

		let tx_ext = crate::extension::AsResources::<T>::new(Some(
			crate::extension::AsResourcesInfo::RegisterFriendRequestForCollection(
				proof,
				0,
				crate::types::MembershipCollection::LitePeople,
			),
		));
		let len = call.encode().len();

		#[block]
		{
			tx_ext
				.test_run(SystemOrigin::None.into(), &call, &Default::default(), len, 0, |_| {
					Ok(Default::default())
				})
				.unwrap()?;
		}

		Ok(())
	}

	#[benchmark]
	fn set_statement_store_account() -> Result<(), BenchmarkError> {
		// Worst case: replacement of an existing entry after the cooldown has elapsed.
		// This exercises the read-existing + revoke-old + insert-new code path.
		<T as Config>::BenchmarkHelper::set_time(Duration::from_secs(
			pallet::SECONDS_PER_DAY + 1000,
		));

		let period = Pallet::<T>::stmt_store_period_from_timestamp(T::Clock::now().as_secs());
		let seq = 0u32;
		let alias: Alias = [10u8; 32];
		let period_key = indiv_support::utils::BigEndianU32::from(period);

		// Pre-populate an existing entry with the same alias and a different target.
		let old_target: T::AccountId = account("old-target", 0, 0);
		let old_seq = 1u32;
		let initial_since = T::Clock::now().as_secs();
		increase_allowance_by(old_target.clone().into(), T::AccountsApiAllowance::get().into());
		StatementStoreAllowances::<T>::insert(
			period_key,
			alias,
			crate::types::StmtStoreAllowanceEntry {
				account_id: old_target.clone(),
				seq: old_seq,
				since: initial_since,
			},
		);
		StmtStoreAllowanceByAccount::<T>::insert(&old_target, (period_key, old_seq, alias), ());

		// Advance past the replacement cooldown.
		let cooldown = T::StmtStoreReplacementCooldown::get() as u64;
		<T as Config>::BenchmarkHelper::set_time(Duration::from_secs(initial_since + cooldown + 1));

		let origin =
			<T as frame_system::Config>::RuntimeOrigin::from(crate::Origin::StmtStoreAlias(alias));
		let target_account: T::AccountId = whitelisted_caller();

		#[extrinsic_call]
		_(origin, period, seq, target_account.clone());

		let entry = StatementStoreAllowances::<T>::get(period_key, alias)
			.expect("allowance entry should be stored");
		assert_eq!(entry.account_id, target_account.clone());
		assert_eq!(entry.seq, seq);
		assert_eq!(
			StmtStoreAllowanceByAccount::<T>::get(&target_account, (period_key, seq, alias)),
			Some(()),
		);
		// Old reverse lookup must be gone.
		assert_eq!(
			StmtStoreAllowanceByAccount::<T>::get(&old_target, (period_key, old_seq, alias)),
			None,
		);
		Ok(())
	}

	#[benchmark]
	fn clear_expired_stmt_store_allowances(
		n: Linear<1, { T::StmtStoreCleanupLimit::get() }>,
	) -> Result<(), BenchmarkError> {
		// Place time in day 0 so we can create entries, then advance past the grace window.
		<T as Config>::BenchmarkHelper::set_time(Duration::from_secs(100));
		let past_period = Pallet::<T>::stmt_store_period_from_timestamp(100);
		let past_period_key = indiv_support::utils::BigEndianU32::from(past_period);

		// Set up `n` entries by calling the extrinsic with fabricated origins.
		for i in 0..n {
			let mut alias = [0u8; 32];
			alias[..4].copy_from_slice(&i.to_le_bytes());
			let origin = <T as frame_system::Config>::RuntimeOrigin::from(
				crate::Origin::StmtStoreAlias(alias),
			);
			let acc: T::AccountId = account("stmt-store", i, 0);
			// Should work because the period checks happen when setting the origin, which we skip
			assert_ok!(Pallet::<T>::set_statement_store_account(origin, past_period, i, acc));
		}

		// Advance time past the period end + grace window.
		let period_end =
			u64::from(past_period.saturating_add(1)).saturating_mul(pallet::SECONDS_PER_DAY);
		let clearable_after = period_end + T::StmtStoreGraceWindow::get() as u64 + 1;
		<T as Config>::BenchmarkHelper::set_time(Duration::from_secs(clearable_after));

		let (_, actual_first_alias) = StatementStoreAllowances::<T>::iter_keys()
			.next()
			.expect("entries were just inserted");

		#[extrinsic_call]
		_(SystemOrigin::Authorized, past_period, actual_first_alias);

		// All `n` entries should have been removed (n <= StmtStoreCleanupLimit).
		assert_eq!(StatementStoreAllowances::<T>::iter_prefix(past_period_key).count(), 0,);
		Ok(())
	}

	#[benchmark]
	fn authorize_clear_expired_stmt_store_allowances() -> Result<(), BenchmarkError> {
		let past_period = 0u32;
		let past_period_key = indiv_support::utils::BigEndianU32::from(past_period);
		let alias: Alias = [20u8; 32];
		let acc: T::AccountId = whitelisted_caller();

		StatementStoreAllowances::<T>::insert(
			past_period_key,
			alias,
			crate::types::StmtStoreAllowanceEntry { account_id: acc.clone(), seq: 0, since: 0 },
		);
		StmtStoreAllowanceByAccount::<T>::insert(&acc, (past_period_key, 0u32, alias), ());

		let period_end = pallet::SECONDS_PER_DAY;
		let clearable_after = period_end + T::StmtStoreGraceWindow::get() as u64 + 1;
		<T as Config>::BenchmarkHelper::set_time(Duration::from_secs(clearable_after));

		let call = Call::<T>::clear_expired_stmt_store_allowances {
			period: past_period,
			first_entry: alias,
		};

		#[block]
		{
			call.authorize(TransactionSource::InBlock)
				.ok_or("Call must give some authorization")??;
		}

		Ok(())
	}

	#[benchmark]
	fn as_stmt_store_allowance_tx_ext() -> Result<(), BenchmarkError> {
		// Worst case: extension validates a replacement proof, which performs the extra
		// storage read of the existing entry to check the cooldown.
		<T as Config>::BenchmarkHelper::set_time(Duration::from_secs(
			pallet::SECONDS_PER_DAY + 1000,
		));

		let (secret, member) = setup_lite_ring_with_one_member::<T>()?;
		let period = Resources::<T>::stmt_store_period_from_timestamp(T::Clock::now().as_secs());
		let seq = 0u32;
		let call = Call::<T>::set_statement_store_account {
			period,
			seq,
			target_account: whitelisted_caller(),
		};
		let call: <T as frame_system::Config>::RuntimeCall = call.into();
		let extension_version = 0u8;

		let msg =
			TxBaseImplication((extension_version, &call)).using_encoded(sp_io::hashing::blake2_256);
		let context = Resources::<T>::stmt_store_slot_context(period, seq);
		let ring_members = <T as indiv_pallet_people::Config>::MemberService::ring_members(
			indiv_pallet_people_lite::LITE_PEOPLE_MEMBER_IDENTIFIER,
			0,
		);
		let capacity: <indiv_pallet_people::CryptoOf<T> as GenerateVerifiable>::Config =
			<T as indiv_pallet_people_lite::Config>::LiteRingExponent::get()
				.try_into()
				.map_err(|_| BenchmarkError::Stop("invalid lite ring exponent"))?;
		let commitment =
			indiv_pallet_people::CryptoOf::<T>::open(capacity, &member, ring_members.into_iter())
				.map_err(|_| BenchmarkError::Stop("failed to open lite commitment"))?;
		let (proof, alias) =
			indiv_pallet_people::CryptoOf::<T>::create(commitment, &secret, &context, &msg)
				.map_err(|_| BenchmarkError::Stop("failed to create lite proof"))?;

		// Pre-populate an existing entry under the same alias so the extension exercises
		// the cooldown branch.
		let period_key = indiv_support::utils::BigEndianU32::from(period);
		let initial_since = T::Clock::now().as_secs();
		let existing_target: T::AccountId = account("existing-target", 0, 0);
		StatementStoreAllowances::<T>::insert(
			period_key,
			alias,
			crate::types::StmtStoreAllowanceEntry {
				account_id: existing_target,
				seq: 1,
				since: initial_since,
			},
		);
		// Advance past the replacement cooldown so validation succeeds.
		let cooldown = T::StmtStoreReplacementCooldown::get() as u64;
		<T as Config>::BenchmarkHelper::set_time(Duration::from_secs(initial_since + cooldown + 1));

		let tx_ext = crate::extension::AsResources::<T>::new(Some(
			crate::extension::AsResourcesInfo::RegisterStatementStoreAllowance(
				proof,
				0,
				crate::types::MembershipCollection::LitePeople,
			),
		));
		let len = call.encode().len();

		#[block]
		{
			tx_ext
				.test_run(SystemOrigin::None.into(), &call, &Default::default(), len, 0, |_| {
					Ok(Default::default())
				})
				.unwrap()?;
		}

		Ok(())
	}

	#[benchmark]
	fn claim_long_term_storage() -> Result<(), BenchmarkError> {
		let period_duration = T::LongTermStoragePeriodDuration::get() as u64;
		<T as Config>::BenchmarkHelper::set_time(Duration::from_secs(period_duration + 100));
		let period =
			Pallet::<T>::long_term_storage_period_from_timestamp(T::Clock::now().as_secs());
		let counter = 0u8;
		let alias: Alias = [42u8; 32];
		let account_id: T::AccountId = whitelisted_caller();

		// Both collection branches go through the same code path inside the pallet (single
		// storage write + one event); the only difference is the constant
		// `LongTermStorageAllocation` value forwarded to `T::LongTermStorageDataStore`,
		// whose cost is metered by that implementor.
		let collection = crate::types::MembershipCollection::People;
		let origin =
			<T as frame_system::Config>::RuntimeOrigin::from(crate::Origin::LongTermStorageClaim {
				alias,
				collection,
				payer: account_id.clone(),
			});

		#[extrinsic_call]
		_(origin, period, counter, account_id.clone());

		assert!(SpentLongTermStorageAliases::<T>::contains_key(
			indiv_support::utils::BigEndianU32::from(period),
			alias,
		));
		assert_last_event::<T>(
			Event::LongTermStorageReserved {
				reservation_id: 0,
				alias,
				period,
				counter,
				account: account_id,
				collection,
			}
			.into(),
		);
		Ok(())
	}

	#[benchmark]
	fn cancel_long_term_storage_reservation() -> Result<(), BenchmarkError> {
		let period_duration = T::LongTermStoragePeriodDuration::get() as u64;
		<T as Config>::BenchmarkHelper::set_time(Duration::from_secs(period_duration + 100));
		let period =
			Pallet::<T>::long_term_storage_period_from_timestamp(T::Clock::now().as_secs());
		let owner: T::AccountId = whitelisted_caller();
		let origin =
			<T as frame_system::Config>::RuntimeOrigin::from(crate::Origin::LongTermStorageClaim {
				alias: [42u8; 32],
				collection: crate::types::MembershipCollection::People,
				payer: owner.clone(),
			});
		Pallet::<T>::claim_long_term_storage(origin, period, 0, owner.clone())?;

		#[extrinsic_call]
		_(SystemOrigin::Signed(owner.clone()), 0);

		assert_last_event::<T>(
			Event::LongTermStorageReservationCancelled { reservation_id: 0, account: owner }.into(),
		);
		Ok(())
	}

	#[benchmark]
	fn expire_long_term_storage_reservations(
		n: Linear<1, { T::LongTermStorageCleanupLimit::get() }>,
	) -> Result<(), BenchmarkError> {
		let period_duration = T::LongTermStoragePeriodDuration::get() as u64;
		<T as Config>::BenchmarkHelper::set_time(Duration::from_secs(period_duration + 100));
		let period =
			Pallet::<T>::long_term_storage_period_from_timestamp(T::Clock::now().as_secs());
		let owner: T::AccountId = whitelisted_caller();
		for i in 0..n {
			let mut alias = [42u8; 32];
			alias[..4].copy_from_slice(&i.to_le_bytes());
			let origin = <T as frame_system::Config>::RuntimeOrigin::from(
				crate::Origin::LongTermStorageClaim {
					alias,
					collection: crate::types::MembershipCollection::LitePeople,
					payer: owner.clone(),
				},
			);
			Pallet::<T>::claim_long_term_storage(origin, period, i as u8, owner.clone())?;
		}
		frame_system::Pallet::<T>::set_block_number(
			T::StorageReservationDuration::get().saturating_add(1u32.into()),
		);

		#[extrinsic_call]
		_(SystemOrigin::Signed(owner), n);

		assert_last_event::<T>(
			Event::LongTermStorageReservationExpired { reservation_id: u64::from(n - 1) }.into(),
		);
		Ok(())
	}

	#[benchmark]
	fn claim_long_term_storage_tx_ext() -> Result<(), BenchmarkError> {
		let period_duration = T::LongTermStoragePeriodDuration::get() as u64;
		<T as Config>::BenchmarkHelper::set_time(Duration::from_secs(period_duration + 100));

		let (secret, member) = setup_lite_ring_with_one_member::<T>()?;

		let period =
			Resources::<T>::long_term_storage_period_from_timestamp(T::Clock::now().as_secs());
		let counter = 0u8;
		let call = Call::<T>::claim_long_term_storage {
			period,
			counter,
			account_id: whitelisted_caller(),
		};
		let call: <T as frame_system::Config>::RuntimeCall = call.into();
		let extension_version = 0u8;

		let msg =
			TxBaseImplication((extension_version, &call)).using_encoded(sp_io::hashing::blake2_256);
		let context = Resources::<T>::long_term_storage_context(period, counter);

		let revision = <T as Config>::MemberService::ring_revision(
			indiv_pallet_people_lite::LITE_PEOPLE_MEMBER_IDENTIFIER,
			0,
		)
		.ok_or(BenchmarkError::Stop("ring revision unavailable"))?;

		let ring_members = <T as indiv_pallet_people::Config>::MemberService::ring_members(
			indiv_pallet_people_lite::LITE_PEOPLE_MEMBER_IDENTIFIER,
			0,
		);
		let capacity: <indiv_pallet_people::CryptoOf<T> as GenerateVerifiable>::Config =
			<T as indiv_pallet_people_lite::Config>::LiteRingExponent::get()
				.try_into()
				.map_err(|_| BenchmarkError::Stop("invalid lite ring exponent"))?;
		let commitment =
			indiv_pallet_people::CryptoOf::<T>::open(capacity, &member, ring_members.into_iter())
				.map_err(|_| BenchmarkError::Stop("failed to open lite commitment"))?;
		let (proof, _) =
			indiv_pallet_people::CryptoOf::<T>::create(commitment, &secret, &context, &msg)
				.map_err(|_| BenchmarkError::Stop("failed to create lite proof"))?;

		let tx_ext = crate::extension::AsResources::<T>::new(Some(
			crate::extension::AsResourcesInfo::ClaimLongTermStorage(
				proof,
				0,
				revision,
				crate::types::MembershipCollection::LitePeople,
			),
		));
		let len = call.encode().len();

		#[block]
		{
			tx_ext
				.test_run(SystemOrigin::None.into(), &call, &Default::default(), len, 0, |_| {
					Ok(Default::default())
				})
				.unwrap()?;
		}

		Ok(())
	}

	#[benchmark]
	fn clear_expired_long_term_storage_aliases(
		n: Linear<1, { T::LongTermStorageCleanupLimit::get() }>,
	) -> Result<(), BenchmarkError> {
		// Place time inside period 0 so the claim dispatches accept it as the current period,
		// then advance past period end + grace window to make it clearable.
		<T as Config>::BenchmarkHelper::set_time(Duration::from_secs(100));
		let past_period: u32 = 0;
		let past_period_key = indiv_support::utils::BigEndianU32::from(past_period);

		// Populate `n` entries by dispatching `claim_long_term_storage` with fabricated
		// origins (mirrors the stmt_store cleanup benchmark — direct storage writes in
		// `#[extrinsic_call]` setup don't reliably persist into the call stage).
		for i in 0..n {
			let mut alias: Alias = [0u8; 32];
			alias[..4].copy_from_slice(&i.to_le_bytes());
			let acc: T::AccountId = account("lts", i, 0);
			let origin = <T as frame_system::Config>::RuntimeOrigin::from(
				crate::Origin::LongTermStorageClaim {
					alias,
					collection: crate::types::MembershipCollection::People,
					payer: acc.clone(),
				},
			);
			assert_ok!(Pallet::<T>::claim_long_term_storage(origin, past_period, 0, acc));
		}

		let period_duration = T::LongTermStoragePeriodDuration::get() as u64;
		let grace = T::LongTermStorageGraceWindow::get() as u64;
		let clearable_after = (u64::from(past_period) + 1) * period_duration + grace + 1;
		<T as Config>::BenchmarkHelper::set_time(Duration::from_secs(clearable_after));

		#[extrinsic_call]
		_(SystemOrigin::Authorized, past_period, n);

		assert_eq!(SpentLongTermStorageAliases::<T>::iter_key_prefix(past_period_key).count(), 0);
		assert_last_event::<T>(
			Event::LongTermStorageAliasesCleared { period: past_period, count: n }.into(),
		);
		Ok(())
	}

	#[benchmark]
	fn authorize_clear_expired_long_term_storage_aliases() -> Result<(), BenchmarkError> {
		let past_period: u32 = 0;
		let alias: Alias = [7u8; 32];
		SpentLongTermStorageAliases::<T>::insert(
			indiv_support::utils::BigEndianU32::from(past_period),
			alias,
			(),
		);

		let period_duration = T::LongTermStoragePeriodDuration::get() as u64;
		let grace = T::LongTermStorageGraceWindow::get() as u64;
		let clearable_after = (u64::from(past_period) + 1) * period_duration + grace + 1;
		<T as Config>::BenchmarkHelper::set_time(Duration::from_secs(clearable_after));

		let call =
			Call::<T>::clear_expired_long_term_storage_aliases { period: past_period, limit: 1 };

		#[block]
		{
			call.authorize(TransactionSource::InBlock)
				.ok_or("Call must give some authorization")??;
		}

		Ok(())
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
