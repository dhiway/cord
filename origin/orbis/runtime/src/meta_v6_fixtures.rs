// This file is part of CORD – https://cord.network

// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

#![allow(dead_code)]

//! Checked-in SCALE vectors for the Orbis v6 signed and sponsored transaction pipelines.
//!
//! These are deliberately binary fixtures. The tests decode every file with `DecodeAll` and
//! independently rebuild it from the runtime types, preventing a text fixture from silently
//! describing a wire format that the runtime does not actually use.

use crate::{AccountId, Runtime, RuntimeCall};
use codec::{DecodeAll, Encode};
use frame_support::{
	dispatch::GetDispatchInfo,
	traits::{BuildGenesisConfig, SignedTransactionBuilder},
};
use sp_core::{ed25519, sr25519, Pair, H256};
use sp_runtime::{
	generic::{Era, SignedPayload},
	traits::{IdentifyAccount, TransactionExtension},
	MultiSignature, MultiSigner,
};
use verifiable::{ring::bandersnatch::BandersnatchVrfVerifiable, GenerateVerifiable};

const CANONICAL_METADATA_IMPLICIT: [u8; 32] = [
	0x85, 0x19, 0x55, 0x76, 0x67, 0xf8, 0x7e, 0xb7, 0xee, 0x32, 0xcd, 0x10, 0x9d, 0x40, 0xac, 0xfd,
	0x48, 0xd9, 0xc5, 0x3a, 0x7e, 0xd9, 0x15, 0xfe, 0x17, 0x33, 0xb0, 0x35, 0x73, 0xab, 0x9e, 0xf9,
];

fn account(pair: &sr25519::Pair) -> AccountId {
	MultiSigner::from(pair.public()).into_account()
}

type MetaBareExtension = (
	crate::meta_v6::ConsumePaidMetaIngress,
	pallet_meta_tx::MetaTxMarker<Runtime>,
	frame_system::CheckNonZeroSender<Runtime>,
	frame_system::CheckSpecVersion<Runtime>,
	frame_system::CheckTxVersion<Runtime>,
	frame_system::CheckGenesis<Runtime>,
	frame_system::CheckMortality<Runtime>,
	frame_system::CheckNonce<Runtime>,
	crate::MetaIdentityBoundPolicies,
	pallet_bulletin_transaction_storage::extension::ValidateStorageCalls<
		Runtime,
		crate::BulletinCallInspector,
	>,
	frame_metadata_hash_extension::CheckMetadataHash<Runtime>,
);

fn meta_tuple(proofs: crate::meta_v6::PolicyProofsV6) -> pallet_meta_tx::MetaTxFor<Runtime> {
	meta_tuple_for(
		RuntimeCall::System(frame_system::Call::remark { remark: b"orbis-v6-meta".to_vec() }),
		proofs,
		None,
		None,
	)
}

fn meta_tuple_for(
	call: RuntimeCall,
	proofs: crate::meta_v6::PolicyProofsV6,
	score: Option<pallet_orbis_score::ScoreAsParticipantData<u32>>,
	honour: Option<pallet_orbis_honour::extension::VoterAuthData<Runtime>>,
) -> pallet_meta_tx::MetaTxFor<Runtime> {
	let pair = ed25519::Pair::from_seed(&[0x42; 32]);
	let signer = MultiSigner::Ed25519(pair.public()).into_account();
	let mortality = frame_system::CheckMortality::<Runtime>::from(Era::Immortal);
	let nonce = frame_system::CheckNonce::<Runtime>::from(0);
	let policy = crate::meta_v6::MetaAccountBoundPoliciesV6::new(proofs);
	let storage = pallet_bulletin_transaction_storage::extension::ValidateStorageCalls::<
		Runtime,
		crate::BulletinCallInspector,
	>::default();
	let metadata =
		frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::decode_all(&mut &[1u8][..])
			.unwrap();
	let metadata_implicit = Some(CANONICAL_METADATA_IMPLICIT);
	let preimage = crate::meta_v6::IntentPreimageV7 {
		domain: crate::meta_v6::META_DOMAIN.to_vec(),
		extension_version: 0,
		genesis_hash: crate::System::block_hash(0),
		spec_version: crate::VERSION.spec_version,
		transaction_version: crate::VERSION.transaction_version,
		inner_signer: signer.clone(),
		call_hash: H256::from(sp_io::hashing::blake2_256(&call.encode())),
		mortality: Era::Immortal,
		nonce: 0,
		policy_proofs_hash: H256::from(sp_io::hashing::blake2_256(&policy.0.encode())),
		storage_extension_hash: H256::from(sp_io::hashing::blake2_256(&storage.encode())),
		metadata_extension_hash: H256::from(sp_io::hashing::blake2_256(&metadata.encode())),
		metadata_implicit,
	};
	let bare: MetaBareExtension = (
		crate::meta_v6::ConsumePaidMetaIngress(preimage),
		pallet_meta_tx::MetaTxMarker::new(),
		frame_system::CheckNonZeroSender::new(),
		frame_system::CheckSpecVersion::new(),
		frame_system::CheckTxVersion::new(),
		frame_system::CheckGenesis::new(),
		mortality,
		nonce,
		(
			pallet_orbis_score::ScoreAsParticipant::<Runtime>::new(score),
			policy,
			pallet_orbis_honour::extension::VoterAuth::<Runtime>::new(honour),
		),
		storage,
		metadata,
	);
	let implicit = (
		bare.0.implicit().unwrap(),
		bare.1.implicit().unwrap(),
		bare.2.implicit().unwrap(),
		bare.3.implicit().unwrap(),
		bare.4.implicit().unwrap(),
		bare.5.implicit().unwrap(),
		bare.6.implicit().unwrap(),
		bare.7.implicit().unwrap(),
		bare.8.implicit().unwrap(),
		bare.9.implicit().unwrap(),
		Some(CANONICAL_METADATA_IMPLICIT),
	);
	let signature = (0u8, call.clone(), bare.clone(), implicit)
		.using_encoded(|payload| pair.sign(&sp_io::hashing::blake2_256(payload)));
	let verify = pallet_verify_signature::VerifySignature::new_with_signature(
		MultiSignature::Ed25519(signature),
		signer,
	);
	let (
		consume,
		marker,
		nonzero,
		spec,
		tx,
		genesis,
		mortality,
		nonce,
		identity_policies,
		storage,
		metadata,
	) = bare;
	pallet_meta_tx::MetaTxFor::<Runtime>::new(
		call,
		0,
		(
			verify,
			consume,
			marker,
			nonzero,
			spec,
			tx,
			genesis,
			mortality,
			nonce,
			identity_policies,
			storage,
			metadata,
		),
	)
}

fn honour_meta_tuples() -> (pallet_meta_tx::MetaTxFor<Runtime>, pallet_meta_tx::MetaTxFor<Runtime>)
{
	use verifiable::{ring::RingDomainSize, GenerateVerifiable};

	let pair = ed25519::Pair::from_seed(&[0x42; 32]);
	let account = MultiSigner::Ed25519(pair.public()).into_account();
	let vote = pallet_orbis_honour::VoteData {
		subject: [0x48; 32],
		point: 7,
		direction: pallet_orbis_honour::Direction::Honourable,
	};
	let call = RuntimeCall::Honour(pallet_orbis_honour::Call::bestow {
		vote: vote.clone(),
		call_valid_from: 0,
	});
	let storage = pallet_bulletin_transaction_storage::extension::ValidateStorageCalls::<
		Runtime,
		crate::BulletinCallInspector,
	>::default();
	let metadata =
		frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::decode_all(&mut &[1u8][..])
			.unwrap();
	// VoterAuth is last inside the nested identity tuple. Its inherited implication is the base
	// Meta call followed by the outer Bulletin/metadata explicit and implicit suffixes.
	let message =
		(0u8, &call, &storage, &metadata, (), Some(CANONICAL_METADATA_IMPLICIT), &account)
			.using_encoded(sp_io::hashing::blake2_256);

	let domain: RingDomainSize = crate::MembersFlexibleRingExponent::get().try_into().unwrap();
	let secret = BandersnatchVrfVerifiable::new_secret([0x48; 32]);
	let member = BandersnatchVrfVerifiable::member_from_secret(&secret);
	let members = core::iter::once(member.clone());
	let commitment = BandersnatchVrfVerifiable::open(domain, &member, members).unwrap();
	let contexts = vote.get_contexts();
	let contexts: Vec<&[u8]> = contexts.iter().map(|context| &context[..]).collect();
	let (proof, _) =
		BandersnatchVrfVerifiable::create_multi_context(commitment, &secret, &contexts, &message)
			.unwrap();
	let valid = meta_tuple_for(
		call.clone(),
		Default::default(),
		None,
		Some(pallet_orbis_honour::extension::VoterAuthData {
			account: account.clone(),
			proof: proof.clone(),
			ring_index: 0,
			revision: 0,
		}),
	);
	let mismatch = meta_tuple_for(
		call,
		Default::default(),
		None,
		Some(pallet_orbis_honour::extension::VoterAuthData {
			account: AccountId::new([0x99; 32]),
			proof,
			ring_index: 0,
			revision: 0,
		}),
	);
	(valid, mismatch)
}

fn policy_vectors() -> [crate::meta_v6::PolicyProofsV6; 7] {
	[
		include_bytes!("../fixtures/meta-v6/proof-person-alias.scale").as_slice(),
		include_bytes!("../fixtures/meta-v6/proof-person-identity.scale").as_slice(),
		include_bytes!("../fixtures/meta-v6/proof-person-alias-revised.scale").as_slice(),
		include_bytes!("../fixtures/meta-v6/proof-lite-person.scale").as_slice(),
		include_bytes!("../fixtures/meta-v6/proof-lite-alias.scale").as_slice(),
		include_bytes!("../fixtures/meta-v6/proof-lite-alias-revised.scale").as_slice(),
		include_bytes!("../fixtures/meta-v6/proof-resources-claim.scale").as_slice(),
	]
	.map(|mut bytes| crate::meta_v6::PolicyProofsV6::decode_all(&mut bytes).unwrap())
}

fn canonical_intent() -> crate::meta_v6::IntentPreimageV7 {
	let tuple = meta_tuple(Default::default());
	let (_, _, extension): (RuntimeCall, u8, crate::MetaTxExtension) =
		DecodeAll::decode_all(&mut tuple.encode().as_slice()).unwrap();
	extension.1 .0
}

fn paid_token() -> crate::meta_v6::PaidMetaTokenV7 {
	crate::meta_v6::PaidMetaTokenV7 {
		payer: AccountId::new([0x42; 32]),
		intent_commitment: canonical_intent().commitment(),
		outer_nonce: 7,
		genesis_hash: crate::System::block_hash(0),
		spec_version: crate::VERSION.spec_version,
		transaction_version: crate::VERSION.transaction_version,
		consumed: false,
	}
}

fn max_envelope_with(meta: pallet_meta_tx::MetaTxFor<Runtime>) -> RuntimeCall {
	let len = meta.encoded_size() as u32;
	let meta = RuntimeCall::MetaTx(pallet_meta_tx::Call::dispatch {
		meta_tx: Box::new(meta),
		meta_tx_encoded_len: len,
	});
	let make = |padding: usize| {
		RuntimeCall::Utility(pallet_utility::Call::batch {
			calls: vec![
				meta.clone(),
				RuntimeCall::System(frame_system::Call::remark { remark: vec![0; padding] }),
			],
		})
	};
	let empty = make(0).encoded_size();
	let target = crate::meta_v6::MAX_META_ENCODED_BYTES.saturating_sub(empty);
	(target.saturating_sub(16)..=target.saturating_add(16))
		.map(make)
		.find(|call| call.encoded_size() == crate::meta_v6::MAX_META_ENCODED_BYTES)
		.expect("an exact 65,536-byte SCALE envelope exists")
}

fn meta_with_intent(
	meta: pallet_meta_tx::MetaTxFor<Runtime>,
	intent: crate::meta_v6::IntentPreimageV7,
) -> pallet_meta_tx::MetaTxFor<Runtime> {
	let (call, version, extension): (RuntimeCall, u8, crate::MetaTxExtension) =
		DecodeAll::decode_all(&mut meta.encode().as_slice()).unwrap();
	let (
		verify,
		_,
		marker,
		nonzero,
		spec,
		tx,
		genesis,
		mortality,
		nonce,
		identity_policies,
		storage,
		metadata,
	) = extension;
	pallet_meta_tx::MetaTxFor::<Runtime>::new(
		call,
		version,
		(
			verify,
			crate::meta_v6::ConsumePaidMetaIngress(intent),
			marker,
			nonzero,
			spec,
			tx,
			genesis,
			mortality,
			nonce,
			identity_policies,
			storage,
			metadata,
		),
	)
}

fn mutations() -> [crate::meta_v6::IntentPreimageV7; 13] {
	let base = canonical_intent();
	let mut domain = base.clone();
	domain.domain.push(0);
	let mut genesis = base.clone();
	genesis.genesis_hash = [1; 32].into();
	let mut spec = base.clone();
	spec.spec_version += 1;
	let mut tx = base.clone();
	tx.transaction_version += 1;
	let mut signer = base.clone();
	signer.inner_signer = AccountId::new([2; 32]);
	let mut call = base.clone();
	call.call_hash = [3; 32].into();
	let mut mortality = base.clone();
	mortality.mortality = Era::mortal(64, 1);
	let mut nonce = base.clone();
	nonce.nonce += 1;
	let mut policy = base.clone();
	policy.policy_proofs_hash = [4; 32].into();
	let mut storage = base.clone();
	storage.storage_extension_hash = [5; 32].into();
	let mut metadata = base.clone();
	metadata.metadata_extension_hash = [6; 32].into();
	let mut metadata_implicit_none = base.clone();
	metadata_implicit_none.metadata_implicit = None;
	let mut metadata_implicit_wrong = base;
	metadata_implicit_wrong.metadata_implicit = Some([7; 32]);
	[
		domain,
		genesis,
		spec,
		tx,
		signer,
		call,
		mortality,
		nonce,
		policy,
		storage,
		metadata,
		metadata_implicit_none,
		metadata_implicit_wrong,
	]
}

fn assert_fixture<T: DecodeAll + Encode + PartialEq + core::fmt::Debug>(bytes: &[u8], expected: T) {
	let decoded = T::decode_all(&mut bytes.as_ref()).expect("fixture is exactly one SCALE value");
	assert_eq!(decoded, expected);
	assert_eq!(bytes, expected.encode());
}

fn verify_meta_signature(meta: pallet_meta_tx::MetaTxFor<Runtime>) -> bool {
	let (call, version, extension): (RuntimeCall, u8, crate::MetaTxExtension) =
		DecodeAll::decode_all(&mut meta.encode().as_slice()).unwrap();
	let (
		verify,
		consume,
		marker,
		nonzero,
		spec,
		tx,
		genesis,
		mortality,
		nonce,
		identity_policies,
		storage,
		metadata,
	) = extension;
	let mirror =
		crate::meta_v6::VerifySignatureMirror::decode_all(&mut verify.encode().as_slice()).unwrap();
	let crate::meta_v6::VerifySignatureMirror::Signed { signature, account } = mirror else {
		return false;
	};
	let bare = (
		consume,
		marker,
		nonzero,
		spec,
		tx,
		genesis,
		mortality,
		nonce,
		identity_policies,
		storage,
		metadata,
	);
	// This is the checked-in, build-produced metadata implicit. Supplying it explicitly keeps the
	// cryptographic fixture test deterministic even in an ordinary developer build where the
	// wasm-builder deliberately leaves `RUNTIME_METADATA_HASH` unset.
	let implicit = (
		bare.0.implicit().unwrap(),
		bare.1.implicit().unwrap(),
		bare.2.implicit().unwrap(),
		bare.3.implicit().unwrap(),
		bare.4.implicit().unwrap(),
		bare.5.implicit().unwrap(),
		bare.6.implicit().unwrap(),
		bare.7.implicit().unwrap(),
		bare.8.implicit().unwrap(),
		bare.9.implicit().unwrap(),
		Some(CANONICAL_METADATA_IMPLICIT),
	);
	let digest = (version, call, bare, implicit).using_encoded(sp_io::hashing::blake2_256);
	sp_runtime::traits::Verify::verify(&signature, digest.as_slice(), &account)
}

fn validate_meta_head(
	meta: pallet_meta_tx::MetaTxFor<Runtime>,
) -> Result<(), sp_runtime::transaction_validity::TransactionValidityError> {
	let (call, version, extension): (RuntimeCall, u8, crate::MetaTxExtension) =
		DecodeAll::decode_all(&mut meta.encode().as_slice()).unwrap();
	let (
		verify,
		consume,
		marker,
		nonzero,
		spec,
		tx,
		genesis,
		mortality,
		nonce,
		identity_policies,
		storage,
		metadata,
	) = extension;
	let bare = (
		consume,
		marker,
		nonzero,
		spec,
		tx,
		genesis,
		mortality,
		nonce,
		identity_policies,
		storage,
		metadata,
	);
	let implicit = (
		bare.0.implicit().unwrap(),
		bare.1.implicit().unwrap(),
		bare.2.implicit().unwrap(),
		bare.3.implicit().unwrap(),
		bare.4.implicit().unwrap(),
		bare.5.implicit().unwrap(),
		bare.6.implicit().unwrap(),
		bare.7.implicit().unwrap(),
		bare.8.implicit().unwrap(),
		bare.9.implicit().unwrap(),
		Some(CANONICAL_METADATA_IMPLICIT),
	);
	verify
		.validate(
			crate::RuntimeOrigin::none(),
			&call,
			&call.get_dispatch_info(),
			call.encoded_size(),
			(),
			&sp_runtime::traits::TxBaseImplication((version, &call, &bare, &implicit)),
			sp_runtime::transaction_validity::TransactionSource::External,
		)
		.map(|_| ())
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct FixtureMetadataResolver;
impl crate::meta_v6::MetadataImplicitResolver for FixtureMetadataResolver {
	fn resolve(
		metadata: &frame_metadata_hash_extension::CheckMetadataHash<Runtime>,
	) -> Result<Option<[u8; 32]>, sp_runtime::transaction_validity::TransactionValidityError> {
		Ok(if metadata.encode() == [0] { None } else { Some(CANONICAL_METADATA_IMPLICIT) })
	}
}

fn assert_outer_fixture(bytes: &[u8], sponsored: bool) {
	let encoded = crate::UncheckedExtrinsic::decode_all(&mut bytes.as_ref()).unwrap();
	assert_eq!(bytes, encoded.encode());
	let transaction = encoded.0;
	let sp_runtime::generic::Preamble::Signed(address, signature, extension) = transaction.preamble
	else {
		panic!("v8 outer fixture must be signed")
	};
	let crate::MultiAddress::Id(signer) = address else { panic!("fixture uses AccountId") };
	if sponsored {
		assert!(matches!(transaction.function, RuntimeCall::MetaTx(..)));
	} else {
		assert!(matches!(transaction.function, RuntimeCall::Score(..)));
	}
	match SignedPayload::new(transaction.function, extension) {
		Ok(payload) => {
			assert!(payload.using_encoded(|payload| {
				sp_runtime::traits::Verify::verify(&signature, payload, &signer)
			}));
		},
		Err(error) => {
			assert!(option_env!("RUNTIME_METADATA_HASH").is_none());
			assert_eq!(
				error,
				sp_runtime::transaction_validity::UnknownTransaction::CannotLookup.into()
			);
		},
	}
}

fn apply_checked_meta_fixture(
	meta: pallet_meta_tx::MetaTxFor<Runtime>,
	sponsor_pair: &sr25519::Pair,
) -> frame_support::dispatch::DispatchResultWithPostInfo {
	let sponsor = MultiSigner::from(sponsor_pair.public()).into_account();
	let meta_len = meta.encoded_size() as u32;
	let call = RuntimeCall::MetaTx(pallet_meta_tx::Call::dispatch {
		meta_tx: Box::new(meta),
		meta_tx_encoded_len: meta_len,
	});
	let payment: crate::PaymentPolicy = pallet_orbis_feeless::ChargeOrSkipFeeless::from(
		pallet_asset_conversion_tx_payment::ChargeAssetTxPayment::<Runtime>::from(0, None),
	)
	.into();
	let extension = crate::paid_tx_extensions(crate::default_inner_tx_extensions(
		crate::System::account_nonce(&sponsor),
		payment,
		Default::default(),
	));
	let payload = SignedPayload::new(call.clone(), extension.clone()).unwrap();
	let signature = payload.using_encoded(|bytes| sponsor_pair.sign(bytes));
	let extrinsic = <crate::UncheckedExtrinsic as SignedTransactionBuilder>::new_signed_transaction(
		call,
		sponsor.into(),
		MultiSignature::Sr25519(signature),
		extension,
	);
	assert!(crate::Executive::validate_transaction(
		sp_runtime::transaction_validity::TransactionSource::External,
		extrinsic.clone(),
		crate::System::block_hash(0),
	)
	.is_ok());
	let outer_result = crate::Executive::apply_extrinsic(extrinsic).unwrap();
	if let Err(error) = outer_result {
		return Err(error.into());
	}
	crate::System::events()
		.into_iter()
		.rev()
		.find_map(|record| match record.event {
			crate::RuntimeEvent::MetaTx(pallet_meta_tx::Event::Dispatched { result }) => {
				Some(result)
			},
			_ => None,
		})
		.unwrap()
}

#[test]
pub(crate) fn checked_in_score_meta_fixture_executes_as_paid_outer_extrinsic() {
	if option_env!("RUNTIME_METADATA_HASH").is_none() {
		return;
	}
	sp_io::TestExternalities::new_empty().execute_with(|| {
		frame_system::GenesisConfig::<Runtime>::default().build();
		crate::System::set_block_number(1);
		crate::System::set_extrinsic_index(0);
		let inner_pair = ed25519::Pair::from_seed(&[0x42; 32]);
		let inner = MultiSigner::Ed25519(inner_pair.public()).into_account();
		let sponsor_pair = sr25519::Pair::from_seed(&[0x24; 32]);
		let sponsor = MultiSigner::from(sponsor_pair.public()).into_account();
		let _ =
			<crate::Balances as frame_support::traits::fungible::Mutate<AccountId>>::set_balance(
				&sponsor,
				100_000_000_000_000,
			);
		crate::Score::onboard_for_recognition(&inner).unwrap();
		let key = pallet_orbis_score::AccountOrPerson::Account(inner.clone());
		pallet_orbis_score::Participants::<Runtime>::mutate(&key, |participant| {
			participant.as_mut().unwrap().score = 10
		});
		let meta = pallet_meta_tx::MetaTxFor::<Runtime>::decode_all(
			&mut include_bytes!("../fixtures/meta-v8/score-participant-meta.scale").as_slice(),
		)
		.unwrap();
		assert!(apply_checked_meta_fixture(meta, &sponsor_pair).is_ok());
		assert_eq!(crate::System::account_nonce(&inner), 1);
		assert_eq!(crate::System::account_nonce(&sponsor), 1);
		assert_eq!(pallet_orbis_score::Participants::<Runtime>::get(&key).unwrap().score, 5);
		assert!(crate::meta_v6::token().is_none());
	});
}

#[test]
pub(crate) fn checked_in_score_nonce_mutation_is_exact_future_without_inner_mutation() {
	if option_env!("RUNTIME_METADATA_HASH").is_none() {
		return;
	}
	sp_io::TestExternalities::new_empty().execute_with(|| {
		frame_system::GenesisConfig::<Runtime>::default().build();
		crate::System::set_block_number(1);
		crate::System::set_extrinsic_index(0);
		let inner_pair = ed25519::Pair::from_seed(&[0x42; 32]);
		let inner = MultiSigner::Ed25519(inner_pair.public()).into_account();
		let sponsor_pair = sr25519::Pair::from_seed(&[0x24; 32]);
		let sponsor = MultiSigner::from(sponsor_pair.public()).into_account();
		let _ =
			<crate::Balances as frame_support::traits::fungible::Mutate<AccountId>>::set_balance(
				&sponsor,
				100_000_000_000_000,
			);
		crate::Score::onboard_for_recognition(&inner).unwrap();
		let key = pallet_orbis_score::AccountOrPerson::Account(inner.clone());
		pallet_orbis_score::Participants::<Runtime>::mutate(&key, |participant| {
			participant.as_mut().unwrap().score = 10
		});
		let before = pallet_orbis_score::Participants::<Runtime>::get(&key);
		let meta = pallet_meta_tx::MetaTxFor::<Runtime>::decode_all(
			&mut include_bytes!("../fixtures/meta-v8/mutate-score-nonce-meta.scale").as_slice(),
		)
		.unwrap();
		assert_eq!(
			apply_checked_meta_fixture(meta, &sponsor_pair),
			Err(pallet_meta_tx::Error::<Runtime>::Future.into())
		);
		assert_eq!(crate::System::account_nonce(&inner), 0);
		assert_eq!(crate::System::account_nonce(&sponsor), 1);
		assert_eq!(pallet_orbis_score::Participants::<Runtime>::get(&key), before);
		assert!(crate::meta_v6::token().is_none());
	});
}

#[test]
pub(crate) fn checked_in_honour_account_mutation_is_exact_bad_signer_and_executable() {
	if option_env!("RUNTIME_METADATA_HASH").is_none() {
		return;
	}
	sp_io::TestExternalities::new_empty().execute_with(|| {
		frame_system::GenesisConfig::<Runtime>::default().build();
		crate::System::set_block_number(1);
		crate::System::set_extrinsic_index(0);
		let bytes = include_bytes!("../fixtures/meta-v8/mutate-honour-account-meta.scale");
		let meta = pallet_meta_tx::MetaTxFor::<Runtime>::decode_all(&mut bytes.as_slice()).unwrap();
		let (call, _, extension): (RuntimeCall, u8, crate::MetaTxExtension) =
			DecodeAll::decode_all(&mut bytes.as_slice()).unwrap();
		let info = call.get_dispatch_info();
		let implicit = extension.implicit().unwrap();
		let error = extension
			.validate(
				crate::RuntimeOrigin::none(),
				&call,
				&info,
				bytes.len(),
				implicit,
				&sp_runtime::traits::TxBaseImplication((0u8, &call)),
				sp_runtime::transaction_validity::TransactionSource::External,
			)
			.err()
			.unwrap();
		assert_eq!(error, sp_runtime::transaction_validity::InvalidTransaction::BadSigner.into());
		let sponsor_pair = sr25519::Pair::from_seed(&[0x24; 32]);
		let sponsor = MultiSigner::from(sponsor_pair.public()).into_account();
		let _ =
			<crate::Balances as frame_support::traits::fungible::Mutate<AccountId>>::set_balance(
				&sponsor,
				100_000_000_000_000,
			);
		assert_eq!(
			apply_checked_meta_fixture(meta, &sponsor_pair),
			Err(pallet_meta_tx::Error::<Runtime>::Invalid.into())
		);
		assert_eq!(crate::System::account_nonce(&sponsor), 1);
		assert!(pallet_orbis_honour::Votes::<Runtime>::iter().next().is_none());
		assert!(crate::meta_v6::token().is_none());
	});
}

fn seed_checked_honour_ring() {
	use indiv_support::traits::{AppendOnlyMembers, RingMode};
	let identifier = *indiv_pallet_people::PEOPLE_MEMBER_IDENTIFIER;
	let domain: verifiable::ring::RingDomainSize =
		crate::MembersFlexibleRingExponent::get().try_into().unwrap();
	let chunks = indiv_support::genesis::ring_verifier_builder_params::<
		verifiable::ring::ark_vrf::suites::bandersnatch::BandersnatchSha512Ell2,
	>(domain);
	for (page_index, page) in chunks.chunks(crate::PeopleChunkPageSize::get() as usize).enumerate()
	{
		let page: frame_support::BoundedVec<
			indiv_pallet_chunks_manager::UncheckedChunk<Runtime>,
			crate::PeopleChunkPageSize,
		> = page
			.iter()
			.cloned()
			.map(indiv_pallet_chunks_manager::UncheckedChunk::<Runtime>)
			.collect::<Vec<_>>()
			.try_into()
			.unwrap();
		indiv_pallet_chunks_manager::Chunks::<Runtime>::insert(
			crate::MembersFlexibleRingExponent::get(),
			page_index as u32,
			page,
		);
	}
	<crate::Members as AppendOnlyMembers>::create_collection(
		xcm::latest::Location::here(),
		&identifier,
		1,
		RingMode::Flexible,
		crate::MembersFlexibleRingExponent::get(),
		None,
	)
	.unwrap();
	let secret = BandersnatchVrfVerifiable::new_secret([0x48; 32]);
	let member = BandersnatchVrfVerifiable::member_from_secret(&secret);
	<crate::Members as AppendOnlyMembers>::add_members(&identifier, vec![member.clone()]).unwrap();
	crate::Members::onboard_members_authorized(
		frame_system::RawOrigin::Authorized.into(),
		identifier,
		0,
		0,
		Some(member),
		0,
	)
	.unwrap();
	crate::Members::build_ring_authorized(
		frame_system::RawOrigin::Authorized.into(),
		identifier,
		0,
		crate::MembersFlexibleRingExponent::get(),
		None,
		1,
		0,
	)
	.unwrap();
}

#[test]
pub(crate) fn checked_in_honour_meta_fixture_executes_against_exact_runtime_ring() {
	if option_env!("RUNTIME_METADATA_HASH").is_none() {
		return;
	}
	sp_io::TestExternalities::new_empty().execute_with(|| {
		frame_system::GenesisConfig::<Runtime>::default().build();
		crate::System::set_block_number(1);
		crate::System::set_extrinsic_index(0);
		seed_checked_honour_ring();
		assert_eq!(
			<crate::Members as indiv_support::traits::MembershipProver>::ring_revision(
				&*indiv_pallet_people::PEOPLE_MEMBER_IDENTIFIER,
				0,
			),
			Some(0),
		);
		let inner_pair = ed25519::Pair::from_seed(&[0x42; 32]);
		let inner = MultiSigner::Ed25519(inner_pair.public()).into_account();
		let sponsor_pair = sr25519::Pair::from_seed(&[0x24; 32]);
		let sponsor = MultiSigner::from(sponsor_pair.public()).into_account();
		// CheckNonce requires a live system account, but the inner Honour signer is not charged.
		let inner_balance = crate::ExistentialDeposit::get();
		let _ =
			<crate::Balances as frame_support::traits::fungible::Mutate<AccountId>>::set_balance(
				&inner,
				inner_balance,
			);
		let _ =
			<crate::Balances as frame_support::traits::fungible::Mutate<AccountId>>::set_balance(
				&sponsor,
				100_000_000_000_000,
			);
		let bytes = include_bytes!("../fixtures/meta-v8/honour-voter-meta.scale");
		let meta = pallet_meta_tx::MetaTxFor::<Runtime>::decode_all(&mut bytes.as_slice()).unwrap();
		let (call, _, extension): (RuntimeCall, u8, crate::MetaTxExtension) =
			DecodeAll::decode_all(&mut bytes.as_slice()).unwrap();
		let auth = extension.9 .2.encode();
		let inner_bytes: &[u8] = inner.as_ref();
		assert_eq!(&auth[1..33], inner_bytes);
		crate::meta_v6::put_token(&crate::meta_v6::PaidMetaTokenV7 {
			payer: sponsor.clone(),
			intent_commitment: extension.1 .0.commitment(),
			outer_nonce: 0,
			genesis_hash: crate::System::block_hash(0),
			spec_version: crate::VERSION.spec_version,
			transaction_version: crate::VERSION.transaction_version,
			consumed: false,
		});
		let implicit = extension.implicit().unwrap();
		let validation = extension.validate(
			crate::RuntimeOrigin::none(),
			&call,
			&call.get_dispatch_info(),
			bytes.len(),
			implicit,
			&sp_runtime::traits::TxBaseImplication((0u8, &call)),
			sp_runtime::transaction_validity::TransactionSource::External,
		);
		crate::meta_v6::clear_token();
		assert!(validation.is_ok(), "checked Honour validation failed: {:?}", validation.err());
		let result = apply_checked_meta_fixture(meta, &sponsor_pair);
		assert!(result.is_ok(), "checked Honour dispatch failed: {result:?}");
		assert_eq!(crate::System::account_nonce(&inner), 1);
		assert_eq!(crate::System::account_nonce(&sponsor), 1);
		assert_eq!(crate::Balances::free_balance(&inner), inner_balance);
		assert!(pallet_orbis_honour::Votes::<Runtime>::iter().next().is_some());
		assert!(crate::meta_v6::token().is_none());
	});
}

#[test]
#[ignore = "run with the checked-in RUNTIME_METADATA_HASH in an isolated target"]
fn regenerate_meta_v8_fixtures() {
	use frame_support::traits::{BuildGenesisConfig, SignedTransactionBuilder};
	sp_io::TestExternalities::new_empty().execute_with(|| {
		frame_system::GenesisConfig::<Runtime>::default().build();
		crate::System::set_block_number(1);
		let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/meta-v8");
		let write = |name: &str, bytes: &[u8]| std::fs::write(dir.join(name), bytes).unwrap();
		let intent = canonical_intent();
		let token = paid_token();
		let meta = meta_tuple(Default::default());
		let score_meta = meta_tuple_for(
			RuntimeCall::Score(pallet_orbis_score::Call::cash_out {}),
			Default::default(),
			Some(pallet_orbis_score::ScoreAsParticipantData { nonce: 0 }),
			None,
		);
		let score_nonce_mismatch_meta = meta_tuple_for(
			RuntimeCall::Score(pallet_orbis_score::Call::cash_out {}),
			Default::default(),
			Some(pallet_orbis_score::ScoreAsParticipantData { nonce: 1 }),
			None,
		);
		let (honour_meta, honour_account_mismatch_meta) = honour_meta_tuples();
		write("intent-preimage.scale", &intent.encode());
		write("intent-commitment.bin", intent.commitment().as_bytes());
		write("paid-token.scale", &token.encode());
		write("paid-token-key.bin", token.key().as_bytes());
		write("verify-consume-tuple.scale", &meta.encode());
		write("score-participant-meta.scale", &score_meta.encode());
		write("mutate-score-nonce-meta.scale", &score_nonce_mismatch_meta.encode());
		write("honour-voter-meta.scale", &honour_meta.encode());
		write("mutate-honour-account-meta.scale", &honour_account_mismatch_meta.encode());
		write("max-envelope.scale", &max_envelope_with(meta.clone()).encode());
		let mutation_names = [
			"mutate-domain.scale",
			"mutate-genesis.scale",
			"mutate-spec.scale",
			"mutate-transaction-version.scale",
			"mutate-signer.scale",
			"mutate-call-hash.scale",
			"mutate-mortality.scale",
			"mutate-nonce.scale",
			"mutate-policy-hash.scale",
			"mutate-storage-hash.scale",
			"mutate-metadata-hash.scale",
			"mutate-metadata-implicit-none.scale",
			"mutate-metadata-implicit-wrong.scale",
		];
		for (name, mutation) in mutation_names.into_iter().zip(mutations()) {
			write(name, &meta_with_intent(meta.clone(), mutation).encode());
		}
		let proof_names = [
			"proof-person-alias-meta.scale",
			"proof-person-identity-meta.scale",
			"proof-person-alias-revised-meta.scale",
			"proof-lite-person-meta.scale",
			"proof-lite-alias-meta.scale",
			"proof-lite-alias-revised-meta.scale",
			"proof-resources-claim-meta.scale",
		];
		for (name, proofs) in proof_names.into_iter().zip(policy_vectors()) {
			write(name, &meta_tuple(proofs).encode());
		}
		let mut wrong_signature = meta.encode();
		let (_, _, extension): (RuntimeCall, u8, crate::MetaTxExtension) =
			DecodeAll::decode_all(&mut wrong_signature.as_slice()).unwrap();
		let mirror =
			crate::meta_v6::VerifySignatureMirror::decode_all(&mut extension.0.encode().as_slice())
				.unwrap();
		let crate::meta_v6::VerifySignatureMirror::Signed { signature, .. } = mirror else {
			panic!("canonical fixture is signed")
		};
		let signature = signature.encode();
		let position = wrong_signature
			.windows(signature.len())
			.position(|window| window == signature)
			.unwrap();
		wrong_signature[position + signature.len() - 1] ^= 1;
		write("mutate-wrong-signature.scale", &wrong_signature);
		let mut trailing = meta.encode();
		trailing.push(0);
		write("mutate-trailing-byte.scale", &trailing);

		let sign_outer = |call: RuntimeCall, pair: &ed25519::Pair| {
			let extension = crate::paid_tx_extensions(crate::default_inner_tx_extensions(
				0,
				pallet_orbis_feeless::ChargeOrSkipFeeless::from(
					pallet_asset_conversion_tx_payment::ChargeAssetTxPayment::<Runtime>::from(
						0, None,
					),
				)
				.into(),
				pallet_revive::evm::tx_extension::SetOrigin::<Runtime>::default(),
			));
			let payload = SignedPayload::new(call.clone(), extension.clone()).unwrap();
			let signature = payload.using_encoded(|bytes| pair.sign(bytes));
			let signer = MultiSigner::Ed25519(pair.public()).into_account();
			<crate::UncheckedExtrinsic as SignedTransactionBuilder>::new_signed_transaction(
				call,
				signer.into(),
				MultiSignature::Ed25519(signature),
				extension,
			)
			.encode()
		};
		let direct_call = RuntimeCall::Score(pallet_orbis_score::Call::cash_out {});
		let extension = crate::paid_tx_extensions(crate::default_inner_tx_extensions(
			0,
			pallet_orbis_feeless::ChargeOrSkipFeeless::from(
				pallet_asset_conversion_tx_payment::ChargeAssetTxPayment::<Runtime>::from(0, None),
			)
			.into(),
			pallet_revive::evm::tx_extension::SetOrigin::<Runtime>::default(),
		));
		let direct_pair = sr25519::Pair::from_string("//Alice", None).unwrap();
		let payload = SignedPayload::new(direct_call.clone(), extension.clone()).unwrap();
		let signature = payload.using_encoded(|bytes| direct_pair.sign(bytes));
		let signer = MultiSigner::Sr25519(direct_pair.public()).into_account();
		let direct =
			<crate::UncheckedExtrinsic as SignedTransactionBuilder>::new_signed_transaction(
				direct_call,
				signer.into(),
				MultiSignature::Sr25519(signature),
				extension,
			)
			.encode();
		write("direct-signed-extrinsic.scale", &direct);
		let sponsored_pair = ed25519::Pair::from_seed(&[0x45; 32]);
		let sponsored_call = RuntimeCall::MetaTx(pallet_meta_tx::Call::dispatch {
			meta_tx: Box::new(meta.clone()),
			meta_tx_encoded_len: meta.encoded_size() as u32,
		});
		write("sponsored-outer-extrinsic.scale", &sign_outer(sponsored_call, &sponsored_pair));
	});
}

#[test]
fn checked_in_meta_v8_fixtures_decode_all_recompute_and_match_hashes() {
	sp_io::TestExternalities::new_empty().execute_with(|| {
		frame_system::GenesisConfig::<Runtime>::default().build();
		crate::System::set_block_number(1);
		let intent = canonical_intent();
		assert_fixture(include_bytes!("../fixtures/meta-v8/intent-preimage.scale"), intent.clone());
		assert_eq!(
			include_bytes!("../fixtures/meta-v8/intent-commitment.bin").as_slice(),
			intent.commitment().as_bytes(),
		);
		let token = paid_token();
		assert_fixture(include_bytes!("../fixtures/meta-v8/paid-token.scale"), token.clone());
		assert_eq!(
			include_bytes!("../fixtures/meta-v8/paid-token-key.bin").as_slice(),
			token.key().as_bytes(),
		);
		let meta_bytes = include_bytes!("../fixtures/meta-v8/verify-consume-tuple.scale");
		assert_eq!(meta_bytes.as_slice(), meta_tuple(Default::default()).encode());
		let meta =
			pallet_meta_tx::MetaTxFor::<Runtime>::decode_all(&mut meta_bytes.as_slice()).unwrap();
		assert_eq!(meta_bytes.as_slice(), meta.encode());
		let (inner_call, _, extension): (RuntimeCall, u8, crate::MetaTxExtension) =
			DecodeAll::decode_all(&mut meta.encode().as_slice()).unwrap();
		assert_eq!(extension.1 .0, intent);
		assert_eq!(extension.1 .0.metadata_implicit, Some(CANONICAL_METADATA_IMPLICIT));
		assert!(extension.1.weight(&inner_call).all_gte(crate::weights::meta_v6::v7_commitment_delta()));
		let score_meta = include_bytes!("../fixtures/meta-v8/score-participant-meta.scale");
		let (score_call, _, score_extension): (RuntimeCall, u8, crate::MetaTxExtension) =
			DecodeAll::decode_all(&mut score_meta.as_slice()).unwrap();
		assert!(matches!(score_call, RuntimeCall::Score(pallet_orbis_score::Call::cash_out {})));
		assert_ne!(score_extension.9 .0.encode(), [0]);
		let score_nonce_mismatch =
			include_bytes!("../fixtures/meta-v8/mutate-score-nonce-meta.scale");
		let (_, _, mismatch_extension): (RuntimeCall, u8, crate::MetaTxExtension) =
			DecodeAll::decode_all(&mut score_nonce_mismatch.as_slice()).unwrap();
		assert_ne!(score_extension.9 .0.encode(), mismatch_extension.9 .0.encode());

		let honour_bytes = include_bytes!("../fixtures/meta-v8/honour-voter-meta.scale");
		let (honour_call, _, honour_extension): (RuntimeCall, u8, crate::MetaTxExtension) =
			DecodeAll::decode_all(&mut honour_bytes.as_slice()).unwrap();
		let mismatch_bytes = include_bytes!("../fixtures/meta-v8/mutate-honour-account-meta.scale");
		let (mismatch_call, _, mismatch_honour_extension):
			(RuntimeCall, u8, crate::MetaTxExtension) =
			DecodeAll::decode_all(&mut mismatch_bytes.as_slice()).unwrap();
		assert_eq!(honour_call, mismatch_call);
		assert!(verify_meta_signature(
			pallet_meta_tx::MetaTxFor::<Runtime>::decode_all(&mut honour_bytes.as_slice()).unwrap()
		));
		assert!(verify_meta_signature(
			pallet_meta_tx::MetaTxFor::<Runtime>::decode_all(&mut mismatch_bytes.as_slice()).unwrap()
		));
		// Ring proofs are randomized even for a deterministic secret. The two checked-in fixtures
		// are generated as a pair, so assert that only the account field changes and the exact same
		// proof/ring/revision suffix is retained. Both complete Meta envelopes remain signed.
		let auth = honour_extension.9 .2.encode();
		let mismatch_auth = mismatch_honour_extension.9 .2.encode();
		assert_eq!(auth[0], 1);
		assert_eq!(mismatch_auth[0], 1);
		assert_ne!(&auth[1..33], &mismatch_auth[1..33]);
		assert_eq!(&auth[33..], &mismatch_auth[33..]);
		assert_ne!(honour_bytes.as_slice(), mismatch_bytes.as_slice());
		let max = max_envelope_with(meta);
		assert_fixture(include_bytes!("../fixtures/meta-v8/max-envelope.scale"), max.clone());
		assert_eq!(max.encoded_size(), crate::meta_v6::MAX_META_ENCODED_BYTES);
		assert!(crate::meta_v6::inspect_paid_meta::<FixtureMetadataResolver>(&max, 0)
			.unwrap()
			.is_some());
		let RuntimeCall::Utility(pallet_utility::Call::batch { mut calls }) = max else {
			unreachable!()
		};
		let RuntimeCall::System(frame_system::Call::remark { remark }) = &mut calls[1] else {
			unreachable!()
		};
		remark.push(0);
		let over = RuntimeCall::Utility(pallet_utility::Call::batch { calls });
		assert_eq!(over.encoded_size(), crate::meta_v6::MAX_META_ENCODED_BYTES + 1);
		assert_eq!(
			crate::meta_v6::inspect_paid_meta::<FixtureMetadataResolver>(&over, 0),
			Err(sp_runtime::transaction_validity::InvalidTransaction::ExhaustsResources.into()),
		);
		let mutation_bytes: [&[u8]; 13] = [
			include_bytes!("../fixtures/meta-v8/mutate-domain.scale"),
			include_bytes!("../fixtures/meta-v8/mutate-genesis.scale"),
			include_bytes!("../fixtures/meta-v8/mutate-spec.scale"),
			include_bytes!("../fixtures/meta-v8/mutate-transaction-version.scale"),
			include_bytes!("../fixtures/meta-v8/mutate-signer.scale"),
			include_bytes!("../fixtures/meta-v8/mutate-call-hash.scale"),
			include_bytes!("../fixtures/meta-v8/mutate-mortality.scale"),
			include_bytes!("../fixtures/meta-v8/mutate-nonce.scale"),
			include_bytes!("../fixtures/meta-v8/mutate-policy-hash.scale"),
			include_bytes!("../fixtures/meta-v8/mutate-storage-hash.scale"),
			include_bytes!("../fixtures/meta-v8/mutate-metadata-hash.scale"),
			include_bytes!("../fixtures/meta-v8/mutate-metadata-implicit-none.scale"),
			include_bytes!("../fixtures/meta-v8/mutate-metadata-implicit-wrong.scale"),
		];
		for (bytes, mutation) in mutation_bytes.into_iter().zip(mutations()) {
			let meta = pallet_meta_tx::MetaTxFor::<Runtime>::decode_all(&mut bytes.as_ref()).unwrap();
			assert_eq!(bytes, meta.encode());
			let (_, _, extension): (RuntimeCall, u8, crate::MetaTxExtension) =
				DecodeAll::decode_all(&mut meta.encode().as_slice()).unwrap();
			assert_eq!(extension.1 .0, mutation);
			assert!(!verify_meta_signature(meta.clone()));
			assert_eq!(
				validate_meta_head(meta.clone()),
				Err(sp_runtime::transaction_validity::InvalidTransaction::BadProof.into()),
			);
			let len = meta.encoded_size() as u32;
			let outer = RuntimeCall::MetaTx(pallet_meta_tx::Call::dispatch {
				meta_tx: Box::new(meta),
				meta_tx_encoded_len: len,
			});
			assert_eq!(
				crate::meta_v6::inspect_paid_meta::<FixtureMetadataResolver>(&outer, 0),
				Err(sp_runtime::transaction_validity::InvalidTransaction::BadProof.into()),
			);
		}
		assert!(mutations().into_iter().all(|mutation| mutation.commitment() != intent.commitment()));

		let proof_bytes: [&[u8]; 7] = [
			include_bytes!("../fixtures/meta-v8/proof-person-alias-meta.scale"),
			include_bytes!("../fixtures/meta-v8/proof-person-identity-meta.scale"),
			include_bytes!("../fixtures/meta-v8/proof-person-alias-revised-meta.scale"),
			include_bytes!("../fixtures/meta-v8/proof-lite-person-meta.scale"),
			include_bytes!("../fixtures/meta-v8/proof-lite-alias-meta.scale"),
			include_bytes!("../fixtures/meta-v8/proof-lite-alias-revised-meta.scale"),
			include_bytes!("../fixtures/meta-v8/proof-resources-claim-meta.scale"),
		];
		for (index, (bytes, expected)) in
			proof_bytes.into_iter().zip(policy_vectors()).enumerate()
		{
			let meta = pallet_meta_tx::MetaTxFor::<Runtime>::decode_all(&mut bytes.as_ref()).unwrap();
			assert_eq!(bytes, meta.encode());
			assert!(verify_meta_signature(meta.clone()));
			let (_, _, extension): (RuntimeCall, u8, crate::MetaTxExtension) =
				DecodeAll::decode_all(&mut meta.encode().as_slice()).unwrap();
			assert_eq!(extension.9 .1 .0, expected);
			assert_eq!(extension.1 .0.metadata_implicit, Some(CANONICAL_METADATA_IMPLICIT));
			match index {
				2 => assert!(matches!(extension.9 .1 .0.personhood, Some(crate::meta_v6::MetaPersonhoodAuthV6::PersonalAliasAccountRevised(ref proof, ..)) if !proof.is_empty())),
				5 => assert!(matches!(extension.9 .1 .0.people_lite, Some(crate::meta_v6::MetaPeopleLiteAuthV6::LiteAliasAccountRevised(ref proof, ..)) if !proof.is_empty())),
				6 => assert!(matches!(extension.9 .1 .0.resources, Some(crate::meta_v6::MetaResourcesAuthV6::ClaimLongTermStorage(ref proof, ..)) if !proof.is_empty())),
				_ => {},
			}
		}

		assert!(verify_meta_signature(
			pallet_meta_tx::MetaTxFor::<Runtime>::decode_all(
				&mut include_bytes!("../fixtures/meta-v8/verify-consume-tuple.scale").as_slice(),
			)
			.unwrap(),
		));
		assert!(validate_meta_head(
			pallet_meta_tx::MetaTxFor::<Runtime>::decode_all(
				&mut include_bytes!("../fixtures/meta-v8/verify-consume-tuple.scale").as_slice(),
			)
			.unwrap(),
		)
		.is_ok());
		let wrong_signature = include_bytes!("../fixtures/meta-v8/mutate-wrong-signature.scale");
		let wrong_signature =
			pallet_meta_tx::MetaTxFor::<Runtime>::decode_all(&mut wrong_signature.as_slice()).unwrap();
		assert!(!verify_meta_signature(wrong_signature.clone()));
		assert_eq!(
			validate_meta_head(wrong_signature),
			Err(sp_runtime::transaction_validity::InvalidTransaction::BadProof.into()),
		);
		assert!(pallet_meta_tx::MetaTxFor::<Runtime>::decode_all(
			&mut include_bytes!("../fixtures/meta-v8/mutate-trailing-byte.scale").as_slice(),
		)
		.is_err());
		assert_outer_fixture(
			include_bytes!("../fixtures/meta-v8/direct-signed-extrinsic.scale"),
			false,
		);
		assert_outer_fixture(
			include_bytes!("../fixtures/meta-v8/sponsored-outer-extrinsic.scale"),
			true,
		);
		assert_eq!(
			include_bytes!("../fixtures/meta-v8/intent-preimage.scale").len(),
			include_bytes!("../fixtures/meta-v6/intent-preimage.scale").len() + 33,
		);
		let digest_hex = |bytes: &[u8]| {
			sp_io::hashing::blake2_256(bytes)
				.iter()
				.map(|byte| format!("{byte:02x}"))
				.collect::<String>()
		};
		assert_eq!(
			digest_hex(include_bytes!("../fixtures/meta-v6/intent-preimage.scale")),
			"cd3df5c81cfd15a2f2debef6b1ee310c67fb5eddcf5c8ae27a05089079edb189"
		);
		assert_eq!(
			digest_hex(include_bytes!("../fixtures/meta-v8/intent-preimage.scale")),
			"e7a525f1e19329e1521c31f6b14185c56c80539cdb6c94e07fd2c067a9f777f4"
		);
		assert_eq!(crate::meta_v6::MAX_META_ENCODED_BYTES, 65_536);
		assert_eq!(crate::meta_v6::MAX_META_PAYLOAD_BYTES, 65_503);
		let manifest: serde_json::Value =
			serde_json::from_str(include_str!("../fixtures/meta-v8/manifest.json")).unwrap();
		assert_eq!(manifest["spec_version"], 29);
		assert_eq!(manifest["transaction_version"], 8);
		assert_eq!(manifest["max_envelope_bytes"], 65_536);
		assert_eq!(manifest["over_bound_error"], "InvalidTransaction::ExhaustsResources");
		assert_eq!(
			manifest["files"]
				.as_array()
				.unwrap()
				.iter()
				.filter(|row| row.get("expected_error").is_some())
				.count(),
			17,
		);
		let expected_error = |file: &str| {
			manifest["files"]
				.as_array()
				.unwrap()
				.iter()
				.find(|row| row["file"] == file)
				.and_then(|row| row["expected_error"].as_str())
		};
		assert_eq!(
			expected_error("mutate-score-nonce-meta.scale"),
			Some("InvalidTransaction::Future")
		);
		assert_eq!(
			expected_error("mutate-honour-account-meta.scale"),
			Some("InvalidTransaction::BadSigner")
		);
		let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/meta-v8");
		for row in manifest["files"].as_array().unwrap() {
			let bytes = std::fs::read(dir.join(row["file"].as_str().unwrap())).unwrap();
			assert_eq!(row["bytes"], bytes.len());
			let hash = sp_io::hashing::sha2_256(&bytes)
				.iter()
				.map(|byte| format!("{byte:02x}"))
				.collect::<String>();
			assert_eq!(row["sha256"], hash);
		}
	});
	if option_env!("RUNTIME_METADATA_HASH").is_some() {
	} else {
	}
}

#[test]
fn checked_in_meta_v6_and_v7_fixtures_are_rejected_after_v8_transition() {
	for bytes in [
		include_bytes!("../fixtures/meta-v6/direct-signed-extrinsic.scale").as_slice(),
		include_bytes!("../fixtures/meta-v7/direct-signed-extrinsic.scale").as_slice(),
	] {
		assert!(crate::UncheckedExtrinsic::decode_all(&mut bytes.as_ref()).is_err());
	}
	let v7 = include_bytes!("../fixtures/meta-v7/verify-consume-tuple.scale");
	assert!(pallet_meta_tx::MetaTxFor::<Runtime>::decode_all(&mut v7.as_slice()).is_err());
	let v6 = include_bytes!("../fixtures/meta-v6/verify-consume-tuple.scale");
	assert!(pallet_meta_tx::MetaTxFor::<Runtime>::decode_all(&mut v6.as_slice()).is_err());
}
