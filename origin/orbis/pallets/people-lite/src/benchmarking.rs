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

//! People lite pallet benchmarks

use super::*;
use alloc::vec::Vec;
use codec::Encode;
use frame_benchmarking::v2::{benchmarks, *};
use frame_support::{
	assert_ok,
	dispatch::{DispatchInfo, RawOrigin},
	traits::{Get, Hooks},
	weights::WeightMeter,
};
use frame_system::RawOrigin as SystemOrigin;
use indiv_support::traits::AppendOnlyMembers;
use sp_runtime::traits::{
	AsSystemOriginSigner, AsTransactionAuthorizedOrigin, DispatchTransaction,
};
use verifiable::GenerateVerifiable;

#[benchmarks(
	where T: Config + core::marker::Send + core::marker::Sync,
	<T as frame_system::Config>::RuntimeCall: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo> + From<Call<T>>,
	<<T as frame_system::Config>::RuntimeCall as Dispatchable>::RuntimeOrigin: AsSystemOriginSigner<T::AccountId> + AsTransactionAuthorizedOrigin + Clone,
	CryptoOf<T>: GenerateVerifiable,
	<CryptoOf<T> as GenerateVerifiable>::Config: TryFrom<indiv_support::traits::RingExponent>,
)]
mod benches {
	use super::*;
	use indiv_support::traits::{ContextualAlias, RevisedContextualAlias};

	type SecretOf<T> = <CryptoOf<T> as GenerateVerifiable>::Secret;
	type CapacityOf<T> = <CryptoOf<T> as GenerateVerifiable>::Config;

	fn lite_member_from<T: Config>(label: &'static [u8], index: u32) -> (SecretOf<T>, MemberOf<T>) {
		let entropy = (label, index).using_encoded(sp_io::hashing::blake2_256);
		let secret = CryptoOf::<T>::new_secret(entropy);
		let member = CryptoOf::<T>::member_from_secret(&secret);
		(secret, member)
	}

	fn create_lite_auth_proof<T: Config>(
		member: &MemberOf<T>,
		secret: &SecretOf<T>,
		msg: &[u8],
	) -> Result<(ProofOf<T>, ContextualAlias), BenchmarkError>
	where
		CapacityOf<T>: TryFrom<indiv_support::traits::RingExponent>,
	{
		let ring_members = T::MemberService::ring_members(crate::LITE_PEOPLE_MEMBER_IDENTIFIER, 0);
		let capacity: CapacityOf<T> = T::LiteRingExponent::get()
			.try_into()
			.ok()
			.expect("ring exponent must convert to capacity");
		let commitment = CryptoOf::<T>::open(capacity, member, ring_members.into_iter())
			.map_err(|_| BenchmarkError::Stop("failed to open lite commitment"))?;
		let (proof, alias) =
			CryptoOf::<T>::create(commitment, secret, &crate::LITE_PEOPLE_AUTH_CONTEXT[..], msg)
				.map_err(|_| BenchmarkError::Stop("failed to create lite proof"))?;
		Ok((proof, ContextualAlias { alias, context: *crate::LITE_PEOPLE_AUTH_CONTEXT }))
	}

	fn setup_lite_auth<T: Config>(
		lite_account: &T::AccountId,
		label: &'static [u8],
	) -> Result<(SecretOf<T>, MemberOf<T>), BenchmarkError>
	where
		CapacityOf<T>: TryFrom<indiv_support::traits::RingExponent>,
	{
		let mut meter = WeightMeter::new();
		Pallet::<T>::on_poll(frame_system::Pallet::<T>::block_number(), &mut meter);
		Pallet::<T>::ensure_lite_collection_created().map_err(|_| {
			BenchmarkError::Stop("lite collection should be initialized by on_poll")
		})?;
		T::MemberService::initialize_chunks(T::LiteRingExponent::get());

		let (lite_secret, lite_member) = lite_member_from::<T>(label, 0);
		crate::LitePeople::<T>::insert(
			lite_account,
			crate::LitePersonInfo {
				ring_vrf_key: lite_member.clone(),
				method: RecognitionMethod::UniqueDevice(whitelisted_caller()),
			},
		);
		frame_system::Pallet::<T>::inc_sufficients(lite_account);

		let cohort_size = T::LiteOnboardingSize::get().max(1);
		let mut members = Vec::with_capacity(cohort_size as usize);
		members.push(lite_member.clone());
		for i in 1..cohort_size {
			members.push(lite_member_from::<T>(label, i).1);
		}
		T::MemberService::add_members(crate::LITE_PEOPLE_MEMBER_IDENTIFIER, members)
			.map_err(|_| BenchmarkError::Stop("failed to add lite members"))?;
		T::MemberService::onboard_all_and_build_ring(crate::LITE_PEOPLE_MEMBER_IDENTIFIER, 0)
			.map_err(|_| BenchmarkError::Stop("failed to build lite ring"))?;

		Ok((lite_secret, lite_member))
	}

	// ============================================================================
	// Tx Extensions
	// ============================================================================

	#[benchmark]
	fn as_lite_person_tx_ext() -> Result<(), BenchmarkError> {
		let lite_account: T::AccountId = whitelisted_caller();
		setup_lite_auth::<T>(&lite_account, b"as-lite-person")?;

		let call: <T as frame_system::Config>::RuntimeCall =
			frame_system::Call::<T>::remark { remark: Vec::new() }.into();
		let len = call.encode().len();

		let tx_ext = crate::PeopleLiteAuth::<T>::new(Some(
			crate::PeopleLiteAuthData::AsLitePerson(0u32.into()),
		));

		let origin = SystemOrigin::Signed(lite_account.clone()).into();

		#[block]
		{
			tx_ext
				.test_run(origin, &call, &Default::default(), len, 0, |_| Ok(Default::default()))
				.unwrap()
				.unwrap();
		}

		assert_eq!(frame_system::Pallet::<T>::account_nonce(&lite_account), 1u32.into());

		Ok(())
	}

	#[benchmark]
	fn as_lite_alias_with_account_tx_ext() -> Result<(), BenchmarkError> {
		let lite_account: T::AccountId = whitelisted_caller();
		let alias_account: T::AccountId = account("alias", 0, 0);
		let (lite_secret, lite_member) =
			setup_lite_auth::<T>(&lite_account, b"as-lite-alias-account")?;

		let setup_call =
			<T as frame_system::Config>::RuntimeCall::from(Call::<T>::set_alias_account {
				account: alias_account.clone(),
				valid_at_block: frame_system::Pallet::<T>::block_number(),
			});
		let setup_msg = (0u8, &setup_call).using_encoded(sp_io::hashing::blake2_256);
		let (_, alias) = create_lite_auth_proof::<T>(&lite_member, &lite_secret, &setup_msg)?;
		let revision = T::MemberService::ring_revision(crate::LITE_PEOPLE_MEMBER_IDENTIFIER, 0)
			.ok_or(BenchmarkError::Stop("lite ring revision missing"))?;
		Pallet::<T>::set_alias_account(
			crate::Origin::<T>::LiteAlias(RevisedContextualAlias { revision, ring: 0, ca: alias })
				.into(),
			alias_account.clone(),
			frame_system::Pallet::<T>::block_number(),
		)
		.map_err(|_| BenchmarkError::Stop("failed to set initial lite alias"))?;

		let call: <T as frame_system::Config>::RuntimeCall =
			frame_system::Call::<T>::remark { remark: Vec::new() }.into();
		let len = call.encode().len();
		let tx_ext = crate::PeopleLiteAuth::<T>::new(Some(
			crate::PeopleLiteAuthData::AsLiteAliasWithAccount(T::Nonce::default()),
		));
		let origin = SystemOrigin::Signed(alias_account.clone()).into();

		#[block]
		{
			tx_ext
				.test_run(origin, &call, &Default::default(), len, 0, |_| Ok(Default::default()))
				.unwrap()
				.unwrap();
		}

		assert_eq!(frame_system::Pallet::<T>::account_nonce(&alias_account), 1u32.into());

		Ok(())
	}

	/// Note: `verify_membership` against the latest ring root is a single validate, with no
	/// loop-over-records to capture.
	#[benchmark]
	fn as_lite_alias_with_proof_tx_ext() -> Result<(), BenchmarkError> {
		let lite_account: T::AccountId = whitelisted_caller();
		let alias_account: T::AccountId = account("alias", 0, 0);
		let (lite_secret, lite_member) = setup_lite_auth::<T>(&lite_account, b"as-lite-proof")?;

		let call = <T as frame_system::Config>::RuntimeCall::from(Call::<T>::set_alias_account {
			account: alias_account.clone(),
			valid_at_block: frame_system::Pallet::<T>::block_number(),
		});
		let len = call.encode().len();
		let msg = (0u8, &call).using_encoded(sp_io::hashing::blake2_256);
		let (proof, _) = create_lite_auth_proof::<T>(&lite_member, &lite_secret, &msg)?;

		// Force the replay check to traverse the full `Some(stored) + decode + comparison`
		// path. Differs from the real `validated_rev_ca` in alias bytes, so equality is
		// false and validate falls through to the success branch.
		let stale_rev_ca = RevisedContextualAlias {
			revision: 0,
			ring: 0,
			ca: ContextualAlias { alias: [0xffu8; 32], context: *crate::LITE_PEOPLE_AUTH_CONTEXT },
		};
		crate::AccountToAlias::<T>::insert(&alias_account, &stale_rev_ca);

		let tx_ext =
			crate::PeopleLiteAuth::<T>::new(Some(crate::PeopleLiteAuthData::AsLiteAliasWithProof(
				proof,
				0,
				*crate::LITE_PEOPLE_AUTH_CONTEXT,
			)));

		#[block]
		{
			tx_ext
				.test_run(SystemOrigin::None.into(), &call, &Default::default(), len, 0, |_| {
					Ok(Default::default())
				})
				.unwrap()
				.unwrap();
		}

		Ok(())
	}

	#[benchmark]
	fn as_lite_alias_with_account_revised_tx_ext() -> Result<(), BenchmarkError> {
		let lite_account: T::AccountId = whitelisted_caller();
		let alias_account: T::AccountId = account("alias", 0, 0);
		let (lite_secret, lite_member) =
			setup_lite_auth::<T>(&lite_account, b"as-lite-alias-revised")?;
		let setup_call =
			<T as frame_system::Config>::RuntimeCall::from(Call::<T>::set_alias_account {
				account: alias_account.clone(),
				valid_at_block: frame_system::Pallet::<T>::block_number(),
			});
		let setup_msg = (0u8, &setup_call).using_encoded(sp_io::hashing::blake2_256);
		let (_, alias) = create_lite_auth_proof::<T>(&lite_member, &lite_secret, &setup_msg)?;
		let stale_revision =
			T::MemberService::ring_revision(crate::LITE_PEOPLE_MEMBER_IDENTIFIER, 0)
				.ok_or(BenchmarkError::Stop("lite ring revision missing"))?;
		Pallet::<T>::set_alias_account(
			crate::Origin::<T>::LiteAlias(RevisedContextualAlias {
				revision: stale_revision,
				ring: 0,
				ca: alias,
			})
			.into(),
			alias_account.clone(),
			frame_system::Pallet::<T>::block_number(),
		)
		.map_err(|_| BenchmarkError::Stop("failed to set initial lite alias"))?;
		let cohort_size = T::LiteOnboardingSize::get().max(1);
		let extra_members = (0..cohort_size)
			.map(|i| lite_member_from::<T>(b"lite-auth-refresh", i).1)
			.collect::<Vec<_>>();
		T::MemberService::add_members(crate::LITE_PEOPLE_MEMBER_IDENTIFIER, extra_members)
			.map_err(|_| BenchmarkError::Stop("failed to add refreshed lite members"))?;
		T::MemberService::onboard_all_and_build_ring(crate::LITE_PEOPLE_MEMBER_IDENTIFIER, 0)
			.map_err(|_| BenchmarkError::Stop("failed to rebuild lite ring"))?;
		let current_revision =
			T::MemberService::ring_revision(crate::LITE_PEOPLE_MEMBER_IDENTIFIER, 0)
				.ok_or(BenchmarkError::Stop("lite ring revision missing"))?;
		assert!(
			current_revision > stale_revision,
			"revised benchmark must refresh to a newer revision",
		);

		let call: <T as frame_system::Config>::RuntimeCall =
			frame_system::Call::<T>::remark { remark: Vec::new() }.into();
		let len = call.encode().len();
		let nonce = T::Nonce::default();
		let inherited_implication = (0u8, &call);
		let msg = (&inherited_implication, "revise", &alias_account, &nonce)
			.using_encoded(sp_io::hashing::blake2_256);
		let (proof, _) = create_lite_auth_proof::<T>(&lite_member, &lite_secret, &msg)?;
		let tx_ext = crate::PeopleLiteAuth::<T>::new(Some(
			crate::PeopleLiteAuthData::AsLiteAliasWithAccountRevised(
				nonce,
				proof,
				0,
				*crate::LITE_PEOPLE_AUTH_CONTEXT,
			),
		));

		let origin = SystemOrigin::Signed(alias_account.clone()).into();

		#[block]
		{
			tx_ext
				.test_run(origin, &call, &Default::default(), len, 0, |_| Ok(Default::default()))
				.unwrap()
				.unwrap();
		}
		assert_eq!(
			crate::AccountToAlias::<T>::get(&alias_account)
				.expect("binding must exist")
				.revision,
			current_revision,
		);
		assert_eq!(frame_system::Pallet::<T>::account_nonce(&alias_account), 1u32.into());

		Ok(())
	}

	// ============================================================================
	// Calls
	// ============================================================================

	#[benchmark]
	fn increase_attestation_allowance() -> Result<(), BenchmarkError> {
		let verifier: T::AccountId = whitelisted_caller();
		let count: u32 = 50;

		#[extrinsic_call]
		_(RawOrigin::Root, verifier.clone(), count);

		assert_eq!(crate::AttestationAllowance::<T>::get(&verifier), count);
		Ok(())
	}

	#[benchmark]
	fn clear_attestation_allowance() -> Result<(), BenchmarkError> {
		let verifier: T::AccountId = whitelisted_caller();

		// Seed some allowance.
		crate::AttestationAllowance::<T>::insert(&verifier, 999_u32);

		#[extrinsic_call]
		_(RawOrigin::Root, verifier.clone());

		assert!(!crate::AttestationAllowance::<T>::contains_key(&verifier));
		Ok(())
	}

	#[benchmark]
	fn register_lite_consumer() -> Result<(), BenchmarkError> {
		let attester: T::AccountId = whitelisted_caller();

		let (account, _) = T::BenchmarkHelper::sign_message(b"mock");
		let identifier_key: CommunicationIdentifier = [0u8; 65];
		let username = Username::try_from(b"validusername.12".to_vec()).unwrap();
		let reserved_username = Some(Username::try_from(b"reservedusername".to_vec()).unwrap());

		let separator_idx = username.iter().position(|b| *b == b'.').unwrap();
		let msg =
			(&account, &attester, &identifier_key, &username[..separator_idx], &reserved_username)
				.encode();
		let (_, signature) = T::BenchmarkHelper::sign_message(&msg[..]);

		let registered_account = account.clone();
		let params = crate::types::LiteConsumerRegistrationParams {
			signature,
			account,
			identifier_key,
			username,
			reserved_username,
		};

		#[block]
		{
			assert_ok!(Pallet::<T>::register_lite_consumer(params, &attester));
		}

		frame_system::Pallet::<T>::assert_last_event(
			crate::Event::<T>::ConsumerRegistered { account: registered_account }.into(),
		);

		Ok(())
	}

	#[benchmark]
	fn attest() -> Result<(), BenchmarkError> {
		let attester: T::AccountId = whitelisted_caller();
		let mut meter = WeightMeter::new();
		Pallet::<T>::on_poll(frame_system::Pallet::<T>::block_number(), &mut meter);
		Pallet::<T>::ensure_lite_collection_created().map_err(|_| {
			BenchmarkError::Stop("lite collection should be initialized by on_poll")
		})?;

		let (att, _) = T::BenchmarkHelper::sign_message(b"mock");
		let sk = CryptoOf::<T>::new_secret([12; 32]);
		let pk = CryptoOf::<T>::member_from_secret(&sk);

		let mut msg = MSG_PREFIX.to_vec();
		msg.extend_from_slice(&att.encode());
		msg.extend_from_slice(&pk.encode());

		let (_, att_sig) = T::BenchmarkHelper::sign_message(&msg[..]);
		let proof_of_ownership = CryptoOf::<T>::sign(&sk, &msg[..]).unwrap();

		// Allowance > 1 forcing the `AttestationAllowance::insert(&verifier, available)` branch.
		crate::AttestationAllowance::<T>::insert(&attester, 2);

		#[extrinsic_call]
		_(RawOrigin::Signed(attester.clone()), att.clone(), att_sig, pk, proof_of_ownership, None);

		assert_eq!(crate::AttestationAllowance::<T>::get(&attester), 1);
		assert!(crate::LitePeople::<T>::contains_key(&att));
		assert!(crate::LitePeopleCollectionCreated::<T>::get());
		frame_system::Pallet::<T>::assert_last_event(
			crate::Event::<T>::PersonAttested { candidate: att, verifier: attester }.into(),
		);
		Ok(())
	}

	#[benchmark]
	fn dispatch_as_signer() -> Result<(), BenchmarkError> {
		let lite_account: T::AccountId = whitelisted_caller();

		let nested: <T as frame_system::Config>::RuntimeCall =
			frame_system::Call::<T>::remark { remark: Vec::new() }.into();

		#[extrinsic_call]
		_(crate::Origin::<T>::LitePerson(lite_account.clone()), Box::new(nested));

		Ok(())
	}

	#[benchmark]
	fn set_alias_account() -> Result<(), BenchmarkError> {
		let lite_account: T::AccountId = whitelisted_caller();
		let old_alias_account: T::AccountId = account("old-alias", 0, 0);
		let new_alias_account: T::AccountId = account("new-alias", 0, 0);
		let (lite_secret, lite_member) = setup_lite_auth::<T>(&lite_account, b"set-alias-account")?;

		// Create a proof and establish the initial alias binding.
		let call = <T as frame_system::Config>::RuntimeCall::from(Call::<T>::set_alias_account {
			account: old_alias_account.clone(),
			valid_at_block: frame_system::Pallet::<T>::block_number(),
		});
		let msg = (0u8, &call).using_encoded(sp_io::hashing::blake2_256);
		let (_, alias) = create_lite_auth_proof::<T>(&lite_member, &lite_secret, &msg)?;
		let revision = T::MemberService::ring_revision(crate::LITE_PEOPLE_MEMBER_IDENTIFIER, 0)
			.ok_or(BenchmarkError::Stop("lite ring revision missing"))?;
		let revised_alias = RevisedContextualAlias { revision, ring: 0, ca: alias.clone() };

		Pallet::<T>::set_alias_account(
			crate::Origin::<T>::LiteAlias(revised_alias.clone()).into(),
			old_alias_account.clone(),
			frame_system::Pallet::<T>::block_number(),
		)
		.map_err(|_| BenchmarkError::Stop("failed to set initial alias binding"))?;

		// Worst case: swap to a different account (dec_sufficients + inc_sufficients).
		#[extrinsic_call]
		_(
			crate::Origin::<T>::LiteAlias(revised_alias.clone()),
			new_alias_account.clone(),
			frame_system::Pallet::<T>::block_number(),
		);

		assert_eq!(crate::AliasToAccount::<T>::get(&alias), Some(new_alias_account.clone()));
		assert_eq!(crate::AccountToAlias::<T>::get(&new_alias_account), Some(revised_alias),);
		assert!(!crate::AccountToAlias::<T>::contains_key(&old_alias_account));
		frame_system::Pallet::<T>::assert_last_event(
			crate::Event::<T>::AliasAccountSet { alias, account: new_alias_account }.into(),
		);
		Ok(())
	}

	#[benchmark]
	fn unset_alias_account() -> Result<(), BenchmarkError> {
		let lite_account: T::AccountId = whitelisted_caller();
		let alias_account: T::AccountId = account("alias", 0, 0);
		let (lite_secret, lite_member) =
			setup_lite_auth::<T>(&lite_account, b"unset-alias-account")?;
		let call = <T as frame_system::Config>::RuntimeCall::from(Call::<T>::set_alias_account {
			account: alias_account.clone(),
			valid_at_block: frame_system::Pallet::<T>::block_number(),
		});
		let msg = (0u8, &call).using_encoded(sp_io::hashing::blake2_256);
		let (_, alias) = create_lite_auth_proof::<T>(&lite_member, &lite_secret, &msg)?;
		let revision = T::MemberService::ring_revision(crate::LITE_PEOPLE_MEMBER_IDENTIFIER, 0)
			.ok_or(BenchmarkError::Stop("lite ring revision missing"))?;
		let revised_alias = RevisedContextualAlias { revision, ring: 0, ca: alias.clone() };

		Pallet::<T>::set_alias_account(
			crate::Origin::<T>::LiteAlias(revised_alias.clone()).into(),
			alias_account.clone(),
			frame_system::Pallet::<T>::block_number(),
		)
		.map_err(|_| BenchmarkError::Stop("failed to set initial lite alias"))?;

		#[extrinsic_call]
		_(crate::Origin::<T>::LiteAlias(revised_alias));

		assert_eq!(crate::AliasToAccount::<T>::get(&alias), None);
		assert_eq!(crate::AccountToAlias::<T>::get(&alias_account), None);
		frame_system::Pallet::<T>::assert_last_event(
			crate::Event::<T>::AliasAccountUnset { alias, account: alias_account }.into(),
		);
		Ok(())
	}

	// ============================================================================
	// Hooks
	// ============================================================================

	#[benchmark]
	fn on_poll_initialize_check_condition() -> Result<(), BenchmarkError> {
		crate::LitePeopleCollectionCreated::<T>::put(true);

		#[block]
		{
			let _ = crate::LitePeopleCollectionCreated::<T>::get();
		}

		Ok(())
	}

	#[benchmark]
	fn on_poll_initialize() -> Result<(), BenchmarkError> {
		crate::LitePeopleCollectionCreated::<T>::kill();

		#[block]
		{
			Pallet::<T>::ensure_lite_collection_exists()
				.map_err(|_| BenchmarkError::Stop("failed to initialize lite collection"))?;
		}

		assert!(crate::LitePeopleCollectionCreated::<T>::get());
		Ok(())
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
