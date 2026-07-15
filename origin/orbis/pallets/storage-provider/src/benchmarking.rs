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

use super::*;
use alloc::{format, vec, vec::Vec};
use codec::Encode;
use frame_benchmarking::v2::*;
use frame_support::traits::{EnsureOrigin, Get, Hooks};
use frame_system::{pallet_prelude::BlockNumberFor, RawOrigin};
use sp_core::ed25519;
use sp_runtime::traits::{Hash as HashT, One, Saturating, Zero};

const SEED: u32 = 0;

fn service_pair(index: u32) -> ed25519_zebra::SigningKey {
	let mut seed = [0u8; 32];
	seed[..4].copy_from_slice(&index.to_le_bytes());
	seed.into()
}

fn service_public(index: u32) -> ed25519::Public {
	let raw: [u8; 32] = ed25519_zebra::VerificationKeyBytes::from(&service_pair(index)).into();
	ed25519::Public::from_raw(raw)
}

fn service_sign(index: u32, message: &[u8]) -> ed25519::Signature {
	ed25519::Signature::from_raw(service_pair(index).sign(message).into())
}

pub trait BenchmarkHelper<T: Config> {
	fn organization(
		provider: &T::AccountId,
		service_key: &ed25519::Public,
		rotation_predecessor: Option<T::Hash>,
	) -> OrganizationRefOf<T>;

	fn set_finalized_block(_: BlockNumberFor<T>) {}

	fn invalidate_authority(_: &T::AccountId, _: &OrganizationRefOf<T>) {}
}

fn admin<T: Config>() -> Result<T::RuntimeOrigin, BenchmarkError> {
	T::AdminOrigin::try_successful_origin()
		.map_err(|_| BenchmarkError::Stop("unable to construct admin origin"))
}

fn provider<T: Config>(index: u32, status: ProviderStatus) -> (T::AccountId, ed25519::Public) {
	if GovernedFinalizedCheckpoint::<T>::get().is_none() {
		GovernedFinalizedCheckpoint::<T>::put(frame_system::Pallet::<T>::block_number());
	}
	let account: T::AccountId = account("provider", index, SEED);
	let key = service_public(index);
	let organization = T::BenchmarkHelper::organization(&account, &key, None);
	let endpoint: EndpointOf<T> = format!("https://provider-{index}.example")
		.into_bytes()
		.try_into()
		.expect("bounded endpoint");
	let now = frame_system::Pallet::<T>::block_number();
	Providers::<T>::insert(
		&account,
		ProviderRecord {
			endpoint: endpoint.clone(),
			organization: organization.clone(),
			service_key: ServiceKeyRecord {
				active: key,
				active_version: 1,
				previous: None,
				pending: None,
				pending_version: None,
				pending_effective_at: None,
			},
			capacity_bytes: u64::MAX / 4,
			allocated_bytes: 0,
			pending_bytes: 0,
			status,
			last_heartbeat: now,
			authority_validated_at: GovernedFinalizedCheckpoint::<T>::get(),
		},
	);
	ProviderIds::<T>::try_mutate(|ids| ids.try_push(account.clone()))
		.expect("provider index has space");
	EndpointOwner::<T>::insert(endpoint, &account);
	ServiceKeyOwner::<T>::insert(key, &account);
	OrganizationHistory::<T>::try_mutate(&account, |items| items.try_push(organization))
		.expect("organization history has space");
	(account, key)
}

fn bucket<T: Config>(
	owner: &T::AccountId,
	primary: &T::AccountId,
	replicas: ReplicasOf<T>,
) -> T::Hash {
	let id = T::Hashing::hash_of(&(b"benchmark-bucket", owner));
	let now = frame_system::Pallet::<T>::block_number();
	Buckets::<T>::insert(
		id,
		BucketRecord {
			owner: owner.clone(),
			version: 1,
			policy: T::Hashing::hash_of(&b"policy"),
			primary: primary.clone(),
			replicas,
			grants: Default::default(),
			created_at: now.saturating_sub(T::CheckpointCadence::get()),
		},
	);
	BucketIds::<T>::try_mutate(|ids| ids.try_push(id)).expect("bucket index has space");
	id
}

fn replicas<T: Config>(count: u32) -> (ReplicasOf<T>, Vec<(T::AccountId, ed25519::Public)>) {
	let items = (0..count)
		.map(|i| provider::<T>(i + 10, ProviderStatus::Active))
		.collect::<Vec<_>>();
	let ids = items
		.iter()
		.map(|item| item.0.clone())
		.collect::<Vec<_>>()
		.try_into()
		.expect("bounded replicas");
	(ids, items)
}

fn agreement<T: Config>(
	owner: T::AccountId,
	primary: T::AccountId,
	replicas: ReplicasOf<T>,
	status: AgreementStatus,
) -> T::Hash {
	let id = T::Hashing::hash_of(&(b"benchmark-agreement", &owner));
	let now = frame_system::Pallet::<T>::block_number();
	Agreements::<T>::insert(
		id,
		AgreementRecord {
			owner,
			bucket_id: T::Hashing::hash_of(&b"bucket"),
			primary,
			replicas,
			bytes: 1,
			created_at: now,
			expires_at: now.saturating_add(T::CheckpointCadence::get()),
			release_at: None,
			version: 1,
			status,
			capacity_state: if status == AgreementStatus::Proposed {
				AgreementCapacityState::Pending
			} else {
				AgreementCapacityState::Allocated
			},
		},
	);
	id
}

fn failover_agreements<T: Config>(
	count: u32,
	bucket_id: T::Hash,
	primary: &T::AccountId,
	replicas: &ReplicasOf<T>,
) {
	let now = frame_system::Pallet::<T>::block_number();
	let mut ids = Vec::new();
	for index in 0..count {
		let agreement_id = T::Hashing::hash_of(&(b"failover-agreement", index));
		Agreements::<T>::insert(
			agreement_id,
			AgreementRecord {
				owner: account("failover-owner", index, SEED),
				bucket_id,
				primary: primary.clone(),
				replicas: replicas.clone(),
				bytes: 1,
				created_at: now,
				expires_at: now.saturating_add(T::CheckpointCadence::get()),
				release_at: None,
				version: 1,
				status: AgreementStatus::Active,
				capacity_state: AgreementCapacityState::Allocated,
			},
		);
		ids.push(agreement_id);
	}
	ProviderAgreements::<T>::insert(
		primary,
		BoundedVec::<T::Hash, T::MaxProviderAgreements>::try_from(ids.clone()).unwrap(),
	);
	BucketAgreements::<T>::insert(
		bucket_id,
		BoundedVec::<T::Hash, T::MaxBucketAgreements>::try_from(ids).unwrap(),
	);
}

fn proof<T: Config>(nodes: u32) -> (MmrProofOf<T>, T::Hash) {
	let leaf = MmrLeafV1 { data_root: T::Hashing::hash_of(&b"leaf"), data_size: 1, total_size: 1 };
	let current = T::Hashing::hash_of(&leaf);
	let peaks = (0..nodes)
		.map(|i| if i + 1 == nodes { current } else { T::Hashing::hash_of(&i) })
		.collect::<Vec<_>>();
	let root = peaks
		.iter()
		.rev()
		.fold(None, |right, peak| {
			Some(match right {
				None => *peak,
				Some(value) => T::Hashing::hash_of(&(*peak, value)),
			})
		})
		.expect("at least one peak");
	(MmrProofV1 { peaks, leaf, leaf_proof: Vec::new() }, root)
}

#[benchmarks]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn register_provider() -> Result<(), BenchmarkError> {
		GovernedFinalizedCheckpoint::<T>::put(frame_system::Pallet::<T>::block_number());
		let account: T::AccountId = account("new-provider", 0, SEED);
		let key = service_public(100);
		let organization = T::BenchmarkHelper::organization(&account, &key, None);
		let endpoint: EndpointOf<T> = b"https://new.example".to_vec().try_into().unwrap();
		#[extrinsic_call]
		_(admin::<T>()?, account, endpoint, key, organization, 1_000_000);
		Ok(())
	}

	#[benchmark]
	fn update_provider() -> Result<(), BenchmarkError> {
		let (who, _) = provider::<T>(0, ProviderStatus::Active);
		let endpoint: EndpointOf<T> = b"https://updated.example".to_vec().try_into().unwrap();
		#[extrinsic_call]
		_(admin::<T>()?, who, endpoint, u64::MAX / 3);
		Ok(())
	}

	#[benchmark]
	fn rotate_service_key() -> Result<(), BenchmarkError> {
		let (who, _) = provider::<T>(0, ProviderStatus::Active);
		let next = service_public(100);
		let organization = T::BenchmarkHelper::organization(&who, &next, None);
		Providers::<T>::mutate(&who, |record| record.as_mut().unwrap().organization = organization);
		#[extrinsic_call]
		_(admin::<T>()?, who, next, GovernedFinalizedCheckpoint::<T>::get().unwrap());
		Ok(())
	}

	#[benchmark]
	fn rotate_provider_organization() -> Result<(), BenchmarkError> {
		let (who, key) = provider::<T>(0, ProviderStatus::Active);
		let old = Providers::<T>::get(&who).unwrap().organization;
		let next = T::BenchmarkHelper::organization(&who, &key, Some(T::Hashing::hash_of(&old)));
		#[extrinsic_call]
		_(admin::<T>()?, who, next);
		Ok(())
	}

	#[benchmark]
	fn set_provider_status() -> Result<(), BenchmarkError> {
		let (who, _) = provider::<T>(0, ProviderStatus::Suspended);
		#[extrinsic_call]
		_(admin::<T>()?, who, ProviderStatus::Active);
		Ok(())
	}

	#[benchmark]
	fn remove_provider() -> Result<(), BenchmarkError> {
		let (who, _) = provider::<T>(0, ProviderStatus::Active);
		#[extrinsic_call]
		_(admin::<T>()?, who);
		Ok(())
	}

	#[benchmark]
	fn heartbeat() {
		let (who, _) = provider::<T>(0, ProviderStatus::Active);
		#[extrinsic_call]
		_(RawOrigin::Signed(who));
	}

	#[benchmark]
	fn create_bucket(r: Linear<2, { T::MaxReplicas::get() }>) {
		let owner: T::AccountId = account("owner", 0, SEED);
		let (primary, _) = provider::<T>(0, ProviderStatus::Active);
		let (replicas, _) = replicas::<T>(r);
		#[extrinsic_call]
		_(RawOrigin::Signed(owner), T::Hashing::hash_of(&b"policy"), primary, replicas);
	}

	#[benchmark]
	fn change_bucket_grant() {
		let owner: T::AccountId = account("owner", 0, SEED);
		let account: T::AccountId = account("grantee", 0, SEED);
		let (primary, _) = provider::<T>(0, ProviderStatus::Active);
		let (replicas, _) = replicas::<T>(2);
		let id = bucket::<T>(&owner, &primary, replicas);
		#[extrinsic_call]
		_(RawOrigin::Signed(owner), id, 1, account, Some(BucketRole::Writer));
	}

	#[benchmark]
	fn propose_agreement(r: Linear<2, { T::MaxReplicas::get() }>) {
		let owner: T::AccountId = account("owner", 0, SEED);
		let (primary, _) = provider::<T>(0, ProviderStatus::Active);
		let (replicas, _) = replicas::<T>(r);
		let id = bucket::<T>(&owner, &primary, replicas);
		let expires =
			frame_system::Pallet::<T>::block_number().saturating_add(T::CheckpointCadence::get());
		#[extrinsic_call]
		_(RawOrigin::Signed(owner), id, 1, 1, expires);
	}

	#[benchmark]
	fn accept_agreement(r: Linear<2, { T::MaxReplicas::get() }>) {
		let owner: T::AccountId = account("owner", 0, SEED);
		let (primary, _) = provider::<T>(0, ProviderStatus::Active);
		let (replicas, _) = replicas::<T>(r);
		Providers::<T>::mutate(&primary, |record| record.as_mut().unwrap().pending_bytes = 1);
		for replica in &replicas {
			Providers::<T>::mutate(replica, |record| record.as_mut().unwrap().pending_bytes = 1);
		}
		let id = agreement::<T>(owner, primary.clone(), replicas, AgreementStatus::Proposed);
		#[extrinsic_call]
		_(RawOrigin::Signed(primary), id, 1);
	}

	#[benchmark]
	fn set_agreement_suspension() -> Result<(), BenchmarkError> {
		let owner: T::AccountId = account("owner", 0, SEED);
		let (primary, _) = provider::<T>(0, ProviderStatus::Active);
		let (replicas, _) = replicas::<T>(4);
		let id = agreement::<T>(owner, primary, replicas, AgreementStatus::Suspended);
		#[extrinsic_call]
		_(admin::<T>()?, id, 1, false);
		Ok(())
	}

	#[benchmark]
	fn terminate_agreement() {
		let owner: T::AccountId = account("owner", 0, SEED);
		let (primary, _) = provider::<T>(0, ProviderStatus::Active);
		let (replicas, _) = replicas::<T>(4);
		let id = agreement::<T>(owner.clone(), primary, replicas, AgreementStatus::Active);
		#[extrinsic_call]
		_(RawOrigin::Signed(owner), id, 1);
	}

	#[benchmark]
	fn expire_agreement() {
		let caller: T::AccountId = account("caller", 0, SEED);
		let owner: T::AccountId = account("owner", 0, SEED);
		let (primary, _) = provider::<T>(0, ProviderStatus::Active);
		let (replicas, _) = replicas::<T>(4);
		let id = agreement::<T>(owner, primary, replicas, AgreementStatus::Active);
		Agreements::<T>::mutate(id, |record| record.as_mut().unwrap().expires_at = Zero::zero());
		#[extrinsic_call]
		_(RawOrigin::Signed(caller), id, 1);
	}

	#[benchmark]
	fn submit_checkpoint(c: Linear<2, { T::MaxReplicas::get() }>) {
		frame_system::Pallet::<T>::set_block_number(T::CheckpointCadence::get());
		T::BenchmarkHelper::set_finalized_block(frame_system::Pallet::<T>::block_number());
		let owner: T::AccountId = account("owner", 0, SEED);
		let (primary, primary_key) = provider::<T>(0, ProviderStatus::Active);
		let (replicas, replica_pairs) = replicas::<T>(c);
		let id = bucket::<T>(&owner, &primary, replicas.clone());
		Pallet::<T>::stage_checkpoint_duty(
			id,
			&primary,
			&replicas,
			Zero::zero(),
			T::CheckpointCadence::get(),
			CheckpointDutyMode::Standard,
			None,
		)
		.expect("benchmark duty is staged");
		let duty = CheckpointDutyPending::<T>::get(id).expect("bucket duty is staged");
		let now = duty.due_at.saturating_add(One::one());
		frame_system::Pallet::<T>::set_block_number(now);
		T::BenchmarkHelper::set_finalized_block(now);
		let payload = CommitmentPayloadV2 {
			version: 2,
			bucket_id: id,
			commitment: CommitmentV1 {
				mmr_root: T::Hashing::hash_of(&b"root"),
				start_seq: 0,
				leaf_count: 1,
			},
			nonce: now,
		};
		let domain: BoundedVec<u8, frame_support::traits::ConstU32<64>> =
			b"cord/storage/checkpoint/v2".to_vec().try_into().unwrap();
		let mut message = domain.to_vec();
		payload.encode_to(&mut message);
		let digest = sp_io::hashing::blake2_256(&message);
		let context = Pallet::<T>::checkpoint_context_for(&payload).unwrap();
		let context_digest = Pallet::<T>::checkpoint_context_digest(&context);
		let mut confirmations = replica_pairs
			.iter()
			.take(2)
			.enumerate()
			.map(|(i, (account, key))| ReplicaSignature {
				provider: account.clone(),
				service_key: *key,
				signature: service_sign(i as u32 + 10, &digest),
				context_signature: service_sign(i as u32 + 10, &context_digest),
			})
			.collect::<Vec<_>>();
		confirmations.sort_by(|left, right| left.provider.encode().cmp(&right.provider.encode()));
		let confirmations: ConfirmationsOf<T> = confirmations.try_into().unwrap();
		#[extrinsic_call]
		_(
			RawOrigin::Signed(primary),
			domain,
			payload,
			duty.due_at,
			duty.grace_until,
			primary_key,
			service_sign(0, &digest),
			service_sign(0, &context_digest),
			confirmations,
		);
	}

	#[benchmark]
	fn promote_checkpoint_fallback(a: Linear<0, { T::MaxBucketAgreements::get() }>) {
		let initial = T::CheckpointCadence::get();
		frame_system::Pallet::<T>::set_block_number(initial);
		GovernedFinalizedCheckpoint::<T>::put(initial);
		T::BenchmarkHelper::set_finalized_block(initial);
		let owner: T::AccountId = account("promotion-owner", 0, SEED);
		let (primary, _) = provider::<T>(0, ProviderStatus::Active);
		let (replicas, replica_pairs) = replicas::<T>(T::MaxReplicas::get());
		let id = bucket::<T>(&owner, &primary, replicas.clone());
		assert_eq!(replicas.len() as u32, T::MaxReplicas::get());
		failover_agreements::<T>(a, id, &primary, &replicas);
		Pallet::<T>::stage_checkpoint_duty(
			id,
			&primary,
			&replicas,
			Zero::zero(),
			initial,
			CheckpointDutyMode::Standard,
			None,
		)
		.expect("benchmark duty is staged");
		let duty = CheckpointDutyPending::<T>::get(id).expect("bucket duty is staged");
		let finalized = duty.grace_until;
		frame_system::Pallet::<T>::set_block_number(finalized);
		GovernedFinalizedCheckpoint::<T>::put(finalized);
		T::BenchmarkHelper::set_finalized_block(finalized);
		let organization = Providers::<T>::get(&primary).unwrap().organization;
		T::BenchmarkHelper::invalidate_authority(&primary, &organization);
		let mut candidates = replica_pairs
			.iter()
			.enumerate()
			.map(|(index, (account, key))| {
				(account.encode(), index as u32 + 10, account.clone(), *key)
			})
			.collect::<Vec<_>>();
		candidates.sort_by(|left, right| left.0.cmp(&right.0));
		let (_, signing_index, promoted, promoted_key) =
			candidates.into_iter().next().expect("at least one replica is configured");
		let duty = Pallet::<T>::checkpoint_duty_at(id, finalized)
			.expect("staged duty is visible at the grace snapshot");
		let payload = CheckpointFallbackPromotionV1 {
			version: 1,
			bucket_id: id,
			snapshot_nonce: finalized,
			duty_id: Pallet::<T>::checkpoint_duty_id(&duty, finalized),
		};
		let signature =
			service_sign(signing_index, &Pallet::<T>::checkpoint_promotion_digest(&payload));
		#[extrinsic_call]
		_(RawOrigin::Signed(promoted.clone()), payload, promoted_key, signature);
		assert_eq!(Buckets::<T>::get(id).unwrap().primary, promoted);
		assert!(BucketSnapshots::<T>::get(id).is_none());
		assert_eq!(
			CheckpointDutyPending::<T>::get(id).unwrap().mode,
			CheckpointDutyMode::PromotionPending
		);
		assert!(CheckpointFallbackPromotionReceiptByBucket::<T>::contains_key(id));
	}

	#[benchmark]
	fn issue_challenge() -> Result<(), BenchmarkError> {
		frame_system::Pallet::<T>::set_block_number(One::one());
		let (primary, _) = provider::<T>(0, ProviderStatus::Active);
		let owner: T::AccountId = account("owner", 0, SEED);
		let (replicas, _) = replicas::<T>(2);
		let id = bucket::<T>(&owner, &primary, replicas.clone());
		BucketSnapshots::<T>::insert(
			id,
			BucketSnapshot {
				commitment: CommitmentV1 {
					mmr_root: T::Hashing::hash_of(&b"root"),
					start_seq: 0,
					leaf_count: 1,
				},
				checkpoint_block: frame_system::Pallet::<T>::block_number(),
				primary_signers: 1,
				commitment_nonce: Zero::zero(),
				replica_confirmations: replicas,
			},
		);
		let due =
			frame_system::Pallet::<T>::block_number().saturating_add(T::CheckpointCadence::get());
		let mut backlog = Vec::new();
		for index in 0..T::MaxChallengeBacklog::get().saturating_sub(1) {
			backlog.push(T::Hashing::hash_of(&(b"queued-challenge", index)));
		}
		ChallengeBacklog::<T>::put(
			BoundedVec::<T::Hash, T::MaxChallengeBacklog>::try_from(backlog)
				.expect("one free challenge backlog slot"),
		);
		#[extrinsic_call]
		_(admin::<T>()?, id, primary, ChunkLocationV1 { leaf_index: 0, chunk_index: 0 }, due);
		Ok(())
	}

	#[benchmark]
	fn submit_challenge_proof(n: Linear<1, { T::MaxProofNodes::get() }>) {
		let (provider, _) = provider::<T>(0, ProviderStatus::Active);
		let (proof, root) = proof::<T>(n);
		let id = T::Hashing::hash_of(&b"challenge");
		Challenges::<T>::insert(
			id,
			ChallengeRecord {
				bucket_id: T::Hashing::hash_of(&b"bucket"),
				provider: provider.clone(),
				expected_commitment: CommitmentV1 { mmr_root: root, start_seq: 0, leaf_count: 1 },
				location: ChunkLocationV1 { leaf_index: 0, chunk_index: 0 },
				due_at: frame_system::Pallet::<T>::block_number().saturating_add(One::one()),
				status: ChallengeStatus::Open,
			},
		);
		let mut backlog = Vec::new();
		for index in 0..T::MaxChallengeBacklog::get().saturating_sub(1) {
			backlog.push(T::Hashing::hash_of(&(b"queued-proof", index)));
		}
		backlog.push(id);
		ChallengeBacklog::<T>::put(
			BoundedVec::<T::Hash, T::MaxChallengeBacklog>::try_from(backlog)
				.expect("saturated challenge backlog"),
		);
		#[extrinsic_call]
		_(RawOrigin::Signed(provider), id, proof);
	}

	#[benchmark]
	fn reconcile_bucket(
		r: Linear<2, { T::MaxReplicas::get() }>,
		a: Linear<0, { T::MaxBucketAgreements::get() }>,
	) {
		let owner: T::AccountId = account("owner", 0, SEED);
		let (primary, _) = provider::<T>(0, ProviderStatus::Suspended);
		let (replicas, _) = replicas::<T>(r);
		let id = bucket::<T>(&owner, &primary, replicas.clone());
		failover_agreements::<T>(a, id, &primary, &replicas);
		for replica in &replicas {
			ReplicaCheckpoint::<T>::insert(id, replica, frame_system::Pallet::<T>::block_number());
		}
		BucketSnapshots::<T>::insert(
			id,
			BucketSnapshot {
				commitment: CommitmentV1 {
					mmr_root: T::Hashing::hash_of(&b"root"),
					start_seq: 0,
					leaf_count: 1,
				},
				checkpoint_block: frame_system::Pallet::<T>::block_number(),
				primary_signers: 1,
				commitment_nonce: Zero::zero(),
				replica_confirmations: replicas,
			},
		);
		let caller: T::AccountId = account("caller", 0, SEED);
		#[extrinsic_call]
		_(RawOrigin::Signed(caller), id);
	}

	#[benchmark]
	fn refresh_bucket_authority_valid(r: Linear<2, { T::MaxReplicas::get() }>) {
		let owner: T::AccountId = account("owner", 0, SEED);
		let (primary, _) = provider::<T>(0, ProviderStatus::Active);
		let (replicas, _) = replicas::<T>(r);
		let id = bucket::<T>(&owner, &primary, replicas);
		let caller: T::AccountId = account("caller", 0, SEED);
		#[extrinsic_call]
		refresh_bucket_authority(RawOrigin::Signed(caller), id);
	}

	#[benchmark]
	fn refresh_bucket_authority_failover(
		r: Linear<2, { T::MaxReplicas::get() }>,
		a: Linear<0, { T::MaxBucketAgreements::get() }>,
	) {
		let owner: T::AccountId = account("owner", 0, SEED);
		let (primary, _) = provider::<T>(0, ProviderStatus::Active);
		let organization = Providers::<T>::get(&primary).unwrap().organization;
		let (replicas, _) = replicas::<T>(r);
		let id = bucket::<T>(&owner, &primary, replicas.clone());
		failover_agreements::<T>(a, id, &primary, &replicas);
		for replica in &replicas {
			ReplicaCheckpoint::<T>::insert(id, replica, frame_system::Pallet::<T>::block_number());
		}
		BucketSnapshots::<T>::insert(
			id,
			BucketSnapshot {
				commitment: CommitmentV1 {
					mmr_root: T::Hashing::hash_of(&b"root"),
					start_seq: 0,
					leaf_count: 1,
				},
				checkpoint_block: frame_system::Pallet::<T>::block_number(),
				primary_signers: 1,
				commitment_nonce: Zero::zero(),
				replica_confirmations: replicas,
			},
		);
		T::BenchmarkHelper::invalidate_authority(&primary, &organization);
		let caller: T::AccountId = account("caller", 0, SEED);
		#[extrinsic_call]
		refresh_bucket_authority(RawOrigin::Signed(caller), id);
	}

	#[benchmark]
	fn register_manifest() {
		let owner: T::AccountId = account("owner", 0, SEED);
		let (primary, _) = provider::<T>(0, ProviderStatus::Active);
		let (replicas, _) = replicas::<T>(2);
		let id = bucket::<T>(&owner, &primary, replicas);
		#[extrinsic_call]
		_(RawOrigin::Signed(owner), id, 1, [1; 32]);
	}

	#[benchmark]
	fn publish_manifest(n: Linear<1, { T::MaxProofNodes::get() }>) {
		let owner: T::AccountId = account("owner", 0, SEED);
		let (primary, _) = provider::<T>(0, ProviderStatus::Active);
		let (replicas, _) = replicas::<T>(2);
		let id = bucket::<T>(&owner, &primary, replicas.clone());
		let (proof, root) = proof::<T>(n);
		BucketSnapshots::<T>::insert(
			id,
			BucketSnapshot {
				commitment: CommitmentV1 { mmr_root: root, start_seq: 0, leaf_count: 1 },
				checkpoint_block: Zero::zero(),
				primary_signers: 1,
				commitment_nonce: Zero::zero(),
				replica_confirmations: replicas,
			},
		);
		CanonicalManifests::<T>::insert(
			[1; 32],
			CanonicalManifestRecord {
				bucket_id: id,
				provider_commitment: None,
				state: CommitmentState::Pending,
				checkpoint: None,
				tombstoned_at: None,
			},
		);
		#[extrinsic_call]
		_(RawOrigin::Signed(primary), [1; 32], 0, proof);
	}

	#[benchmark]
	fn tombstone_manifest() {
		let owner: T::AccountId = account("owner", 0, SEED);
		let (primary, _) = provider::<T>(0, ProviderStatus::Active);
		let (replicas, _) = replicas::<T>(4);
		let id = bucket::<T>(&owner, &primary, replicas);
		CanonicalManifests::<T>::insert(
			[1; 32],
			CanonicalManifestRecord {
				bucket_id: id,
				provider_commitment: Some([2; 32]),
				state: CommitmentState::Publishable,
				checkpoint: None,
				tombstoned_at: None,
			},
		);
		#[extrinsic_call]
		_(RawOrigin::Signed(owner), [1; 32]);
	}

	#[benchmark]
	fn acknowledge_manifest_deletion() {
		let (provider, key) = provider::<T>(0, ProviderStatus::Active);
		let bucket_id = T::Hashing::hash_of(&b"bucket");
		let manifest = [1; 32];
		let evidence = T::Hashing::hash_of(&b"evidence");
		let at = GovernedFinalizedCheckpoint::<T>::get().unwrap();
		CanonicalManifests::<T>::insert(
			manifest,
			CanonicalManifestRecord {
				bucket_id,
				provider_commitment: Some([2; 32]),
				state: CommitmentState::Tombstoned,
				checkpoint: None,
				tombstoned_at: Some(at),
			},
		);
		let required: AssignedProvidersOf<T> = vec![provider.clone()].try_into().unwrap();
		ManifestDeletionRequirements::<T>::insert(manifest, required);
		let digest = T::Hashing::hash_of(&(
			b"cord/storage/deletion-ack/v1",
			bucket_id,
			manifest,
			evidence,
			at,
		));
		let encoded = digest.encode();
		let mut bytes = [0u8; 32];
		bytes.copy_from_slice(&encoded);
		#[extrinsic_call]
		_(RawOrigin::Signed(provider), manifest, evidence, key, service_sign(0, &bytes));
	}

	#[benchmark]
	fn replace_bucket_replica(a: Linear<0, { T::MaxBucketAgreements::get() }>) {
		let owner: T::AccountId = account("owner", 0, SEED);
		let (primary, _) = provider::<T>(0, ProviderStatus::Active);
		let (old, _) = provider::<T>(1, ProviderStatus::Suspended);
		let (new, _) = provider::<T>(2, ProviderStatus::Active);
		let (mut replicas, _) = replicas::<T>(2);
		replicas[0] = old.clone();
		let id = bucket::<T>(&owner, &primary, replicas.clone());
		ProviderBucketAssignmentCount::<T>::insert(&old, 1);
		let mut ids = Vec::new();
		let mut pending = 0u64;
		let mut allocated = 0u64;
		for i in 0..a {
			let agreement_owner: T::AccountId = account("replacement-agreement", i, SEED);
			let status =
				if i % 2 == 0 { AgreementStatus::Proposed } else { AgreementStatus::Active };
			let agreement_id =
				agreement::<T>(agreement_owner, primary.clone(), replicas.clone(), status);
			Agreements::<T>::mutate(agreement_id, |record| {
				let record = record.as_mut().unwrap();
				record.bucket_id = id;
				record.bytes = 1;
			});
			if status == AgreementStatus::Proposed {
				pending = pending.saturating_add(1);
			} else {
				allocated = allocated.saturating_add(1);
			}
			ids.push(agreement_id);
		}
		ProviderAgreements::<T>::insert(
			&old,
			BoundedVec::<T::Hash, T::MaxProviderAgreements>::try_from(ids.clone())
				.expect("bounded provider agreement index"),
		);
		BucketAgreements::<T>::insert(
			id,
			BoundedVec::<T::Hash, T::MaxBucketAgreements>::try_from(ids)
				.expect("bounded bucket agreement index"),
		);
		Providers::<T>::mutate(&old, |record| {
			let record = record.as_mut().unwrap();
			record.pending_bytes = pending;
			record.allocated_bytes = allocated;
		});
		#[extrinsic_call]
		_(RawOrigin::Signed(owner), id, 1, old, new);
	}

	#[benchmark]
	fn advance_finalized_checkpoint() -> Result<(), BenchmarkError> {
		let one: BlockNumberFor<T> = One::one();
		let now = one.saturating_add(one);
		frame_system::Pallet::<T>::set_block_number(now);
		GovernedFinalizedCheckpoint::<T>::put(one);
		#[extrinsic_call]
		_(admin::<T>()?, now);
		Ok(())
	}

	#[benchmark]
	fn on_initialize_release(r: Linear<0, { T::MaxCapacityReleasesPerBlock::get() }>) {
		let one: BlockNumberFor<T> = One::one();
		let now = one.saturating_add(one).saturating_add(one);
		frame_system::Pallet::<T>::set_block_number(now);
		let mut release_ids = Vec::new();
		for i in 0..r {
			let owner: T::AccountId = account("release-owner", i, SEED);
			let (primary, _) = provider::<T>(i + 1000, ProviderStatus::Active);
			let id = agreement::<T>(
				owner,
				primary.clone(),
				Default::default(),
				AgreementStatus::Cancelled,
			);
			Agreements::<T>::mutate(id, |record| record.as_mut().unwrap().release_at = Some(now));
			ProviderAgreements::<T>::try_mutate(&primary, |ids| ids.try_push(id))
				.expect("agreement index has space");
			release_ids.push(id);
		}
		CapacityReleases::<T>::insert(
			now,
			BoundedVec::try_from(release_ids).expect("bounded release queue"),
		);
		#[block]
		{
			Pallet::<T>::on_initialize(now);
		}
	}

	#[benchmark]
	fn on_initialize_reconcile(
		q: Linear<0, { T::MaxReconciliationRecords::get() }>,
		a: Linear<0, { T::MaxBucketAgreements::get() }>,
	) {
		let one: BlockNumberFor<T> = One::one();
		let now = one.saturating_add(one).saturating_add(one).saturating_add(one);
		frame_system::Pallet::<T>::set_block_number(now);
		GovernedFinalizedCheckpoint::<T>::put(now);
		let (bucket_primary, _) = provider::<T>(10_000, ProviderStatus::Active);
		ProviderIds::<T>::kill();
		let provider_limit = T::MaxReconciliationRecords::get().saturating_add(1) / 2;
		let provider_records = q.saturating_sub(1).min(provider_limit);
		let mut reconcile_provider_ids = Vec::new();
		for i in 0..provider_records {
			let (provider, _) = provider::<T>(i + 2000, ProviderStatus::Active);
			reconcile_provider_ids.push(provider.clone());
			Providers::<T>::mutate(provider, |record| {
				record.as_mut().unwrap().organization.valid_until = now
			});
		}
		if q > 0 {
			let owner: T::AccountId = account("reconcile-failover-owner", 0, SEED);
			let (primary, _) = provider::<T>(30_000, ProviderStatus::Suspended);
			let (replicas, _) = replicas::<T>(2);
			let id = bucket::<T>(&owner, &primary, replicas.clone());
			for replica in &replicas {
				ReplicaCheckpoint::<T>::insert(id, replica, now);
			}
			BucketSnapshots::<T>::insert(
				id,
				BucketSnapshot {
					commitment: CommitmentV1 {
						mmr_root: T::Hashing::hash_of(&b"reconcile-root"),
						start_seq: 0,
						leaf_count: 1,
					},
					checkpoint_block: now,
					primary_signers: 1,
					commitment_nonce: now,
					replica_confirmations: replicas.clone(),
				},
			);
			failover_agreements::<T>(a, id, &primary, &replicas);
		}
		for i in provider_records.saturating_add(1)..q {
			let owner: T::AccountId = account("reconcile-owner", i, SEED);
			let _ = bucket::<T>(&owner, &bucket_primary, Default::default());
		}
		ProviderIds::<T>::put(
			BoundedVec::<T::AccountId, T::MaxProviders>::try_from(reconcile_provider_ids).unwrap(),
		);
		#[block]
		{
			Pallet::<T>::on_initialize(now);
		}
	}

	#[benchmark]
	fn on_initialize_challenges(c: Linear<0, { T::MaxChallengesPerBlock::get() }>) {
		let one: BlockNumberFor<T> = One::one();
		let now = one
			.saturating_add(one)
			.saturating_add(one)
			.saturating_add(one)
			.saturating_add(one);
		frame_system::Pallet::<T>::set_block_number(now);
		GovernedFinalizedCheckpoint::<T>::put(now);
		let (provider, _) = provider::<T>(20_000, ProviderStatus::Active);
		let mut ids = Vec::new();
		for i in 0..T::MaxChallengeBacklog::get() {
			let challenge_id = T::Hashing::hash_of(&(b"due-challenge", i));
			Challenges::<T>::insert(
				challenge_id,
				ChallengeRecord {
					bucket_id: T::Hashing::hash_of(&(b"challenge-bucket", i)),
					provider: provider.clone(),
					expected_commitment: CommitmentV1 {
						mmr_root: T::Hashing::hash_of(&b"root"),
						start_seq: 0,
						leaf_count: 1,
					},
					location: ChunkLocationV1 { leaf_index: 0, chunk_index: 0 },
					due_at: now.saturating_sub(one),
					status: ChallengeStatus::Open,
				},
			);
			ids.push(challenge_id);
		}
		ChallengeBacklog::<T>::put(BoundedVec::try_from(ids.clone()).unwrap());
		#[block]
		{
			Pallet::<T>::process_due_challenges_with_limit(c);
		}
		assert_eq!(ChallengeBacklog::<T>::get().len(), ids.len().saturating_sub(c as usize));
		assert_eq!(
			ids.into_iter()
				.filter(|challenge_id| Challenges::<T>::get(challenge_id).unwrap().status ==
					ChallengeStatus::TimedOut)
				.count(),
			c as usize,
		);
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
