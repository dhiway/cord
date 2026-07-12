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
use frame_support::traits::BuildGenesisConfig;
use sp_core::{sr25519, Pair, H256};
use sp_runtime::{
	generic::{Era, SignedPayload},
	traits::{IdentifyAccount, TransactionExtension},
	MultiSignature, MultiSigner,
};

const CANONICAL_METADATA_IMPLICIT: [u8; 32] = [0x28; 32];

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
	crate::meta_v6::MetaAccountBoundPoliciesV6,
	pallet_bulletin_transaction_storage::extension::ValidateStorageCalls<
		Runtime,
		crate::BulletinCallInspector,
	>,
	frame_metadata_hash_extension::CheckMetadataHash<Runtime>,
);

fn meta_tuple(proofs: crate::meta_v6::PolicyProofsV6) -> pallet_meta_tx::MetaTxFor<Runtime> {
	let pair = sr25519::Pair::from_string("//Alice", None).unwrap();
	let signer = account(&pair);
	let call =
		RuntimeCall::System(frame_system::Call::remark { remark: b"orbis-v6-meta".to_vec() });
	let mortality = frame_system::CheckMortality::<Runtime>::from(Era::Immortal);
	let nonce = frame_system::CheckNonce::<Runtime>::from(0);
	let policy = crate::meta_v6::MetaAccountBoundPoliciesV6::new(proofs);
	let storage = pallet_bulletin_transaction_storage::extension::ValidateStorageCalls::<
		Runtime,
		crate::BulletinCallInspector,
	>::default();
	let metadata =
		frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::new_with_custom_hash(
			CANONICAL_METADATA_IMPLICIT,
		);
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
		policy,
		storage,
		metadata,
	);
	let implicit = bare.implicit().unwrap();
	let signature = (0u8, call.clone(), bare.clone(), implicit)
		.using_encoded(|payload| pair.sign(&sp_io::hashing::blake2_256(payload)));
	let verify = pallet_verify_signature::VerifySignature::new_with_signature(
		MultiSignature::Sr25519(signature),
		signer,
	);
	let (consume, marker, nonzero, spec, tx, genesis, mortality, nonce, policy, storage, metadata) =
		bare;
	pallet_meta_tx::MetaTxFor::<Runtime>::new(
		call,
		0,
		(
			verify, consume, marker, nonzero, spec, tx, genesis, mortality, nonce, policy, storage,
			metadata,
		),
	)
}

fn policy_vectors() -> [crate::meta_v6::PolicyProofsV6; 7] {
	let empty_proof: indiv_pallet_people::types::ProofOf<Runtime> =
		Vec::<u8>::new().try_into().unwrap();
	[
		crate::meta_v6::PolicyProofsV6 {
			personhood: Some(crate::meta_v6::MetaPersonhoodAuthV6::PersonalAliasAccount),
			..Default::default()
		},
		crate::meta_v6::PolicyProofsV6 {
			personhood: Some(crate::meta_v6::MetaPersonhoodAuthV6::PersonalIdentityAccount),
			..Default::default()
		},
		crate::meta_v6::PolicyProofsV6 {
			personhood: Some(crate::meta_v6::MetaPersonhoodAuthV6::PersonalAliasAccountRevised(
				empty_proof.clone(),
				0,
				[0x31; 32],
			)),
			..Default::default()
		},
		crate::meta_v6::PolicyProofsV6 {
			people_lite: Some(crate::meta_v6::MetaPeopleLiteAuthV6::LitePerson),
			..Default::default()
		},
		crate::meta_v6::PolicyProofsV6 {
			people_lite: Some(crate::meta_v6::MetaPeopleLiteAuthV6::LiteAliasAccount),
			..Default::default()
		},
		crate::meta_v6::PolicyProofsV6 {
			people_lite: Some(crate::meta_v6::MetaPeopleLiteAuthV6::LiteAliasAccountRevised(
				empty_proof.clone(),
				0,
				[0x32; 32],
			)),
			..Default::default()
		},
		crate::meta_v6::PolicyProofsV6 {
			resources: Some(crate::meta_v6::MetaResourcesAuthV6::ClaimLongTermStorage(
				empty_proof,
				0,
				1,
				indiv_pallet_resources::types::MembershipCollection::People,
			)),
			..Default::default()
		},
	]
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
	let mut nested = RuntimeCall::MetaTx(pallet_meta_tx::Call::dispatch {
		meta_tx: Box::new(meta),
		meta_tx_encoded_len: len,
	});
	for _ in 0..3 {
		nested = RuntimeCall::Utility(pallet_utility::Call::batch { calls: vec![nested] });
	}
	let mut calls = vec![nested];
	for n in 0..27 {
		calls.push(RuntimeCall::System(frame_system::Call::remark { remark: vec![n] }));
	}
	RuntimeCall::Utility(pallet_utility::Call::batch { calls })
}

fn mutations() -> [crate::meta_v6::IntentPreimageV7; 12] {
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
	let mut metadata_implicit = base;
	metadata_implicit.metadata_implicit = Some([7; 32]);
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
		metadata_implicit,
	]
}

fn assert_fixture<T: DecodeAll + Encode + PartialEq + core::fmt::Debug>(bytes: &[u8], expected: T) {
	let decoded = T::decode_all(&mut bytes.as_ref()).expect("fixture is exactly one SCALE value");
	assert_eq!(decoded, expected);
	assert_eq!(bytes, expected.encode());
}

#[test]
fn checked_in_meta_v7_fixtures_decode_all_recompute_and_match_hashes() {
	sp_io::TestExternalities::new_empty().execute_with(|| {
		frame_system::GenesisConfig::<Runtime>::default().build();
		crate::System::set_block_number(1);
		let intent = canonical_intent();
		assert_fixture(include_bytes!("../fixtures/meta-v7/intent-preimage.scale"), intent.clone());
		assert_eq!(
			include_bytes!("../fixtures/meta-v7/intent-commitment.bin").as_slice(),
			intent.commitment().as_bytes(),
		);
		let token = paid_token();
		assert_fixture(include_bytes!("../fixtures/meta-v7/paid-token.scale"), token.clone());
		assert_eq!(
			include_bytes!("../fixtures/meta-v7/paid-token-key.bin").as_slice(),
			token.key().as_bytes(),
		);
		let meta_bytes = include_bytes!("../fixtures/meta-v7/verify-consume-tuple.scale");
		assert_eq!(meta_bytes.as_slice(), meta_tuple(Default::default()).encode());
		let meta =
			pallet_meta_tx::MetaTxFor::<Runtime>::decode_all(&mut meta_bytes.as_slice()).unwrap();
		assert_eq!(meta_bytes.as_slice(), meta.encode());
		let (_, _, extension): (RuntimeCall, u8, crate::MetaTxExtension) =
			DecodeAll::decode_all(&mut meta.encode().as_slice()).unwrap();
		assert_eq!(extension.1 .0, intent);
		assert_eq!(extension.1 .0.metadata_implicit, Some(CANONICAL_METADATA_IMPLICIT));
		assert_fixture(
			include_bytes!("../fixtures/meta-v7/max-envelope.scale"),
			max_envelope_with(meta),
		);
		let mutation_bytes: [&[u8]; 12] = [
			include_bytes!("../fixtures/meta-v7/mutate-domain.scale"),
			include_bytes!("../fixtures/meta-v7/mutate-genesis.scale"),
			include_bytes!("../fixtures/meta-v7/mutate-spec.scale"),
			include_bytes!("../fixtures/meta-v7/mutate-transaction-version.scale"),
			include_bytes!("../fixtures/meta-v7/mutate-signer.scale"),
			include_bytes!("../fixtures/meta-v7/mutate-call-hash.scale"),
			include_bytes!("../fixtures/meta-v7/mutate-mortality.scale"),
			include_bytes!("../fixtures/meta-v7/mutate-nonce.scale"),
			include_bytes!("../fixtures/meta-v7/mutate-policy-hash.scale"),
			include_bytes!("../fixtures/meta-v7/mutate-storage-hash.scale"),
			include_bytes!("../fixtures/meta-v7/mutate-metadata-hash.scale"),
			include_bytes!("../fixtures/meta-v7/mutate-metadata-implicit.scale"),
		];
		for (bytes, mutation) in mutation_bytes.into_iter().zip(mutations()) {
			assert_fixture(bytes, mutation);
		}
		assert_eq!(
			include_bytes!("../fixtures/meta-v7/intent-preimage.scale").len(),
			include_bytes!("../fixtures/meta-v6/intent-preimage.scale").len() + 33,
		);
		let manifest: serde_json::Value =
			serde_json::from_str(include_str!("../fixtures/meta-v7/manifest.json")).unwrap();
		assert_eq!(manifest["spec_version"], 28);
		assert_eq!(manifest["transaction_version"], 7);
		let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/meta-v7");
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
}

#[test]
fn checked_in_meta_v6_fixtures_are_hard_rejected_after_v7_transition() {
	sp_io::TestExternalities::new_empty().execute_with(|| {
		frame_system::GenesisConfig::<Runtime>::default().build();
		crate::System::set_block_number(1);
		let direct_bytes = include_bytes!("../fixtures/meta-v6/direct-signed-extrinsic.scale");
		let direct = crate::UncheckedExtrinsic::decode_all(&mut direct_bytes.as_slice()).unwrap().0;
		let sp_runtime::generic::Preamble::Signed(address, signature, extension) = direct.preamble
		else {
			panic!("legacy direct fixture must be signed")
		};
		let crate::MultiAddress::Id(signer) = address else { panic!("fixture uses AccountId") };
		let payload = SignedPayload::new(direct.function, extension).unwrap();
		assert!(!payload
			.using_encoded(|bytes| sp_runtime::traits::Verify::verify(&signature, bytes, &signer)));

		let meta_bytes = include_bytes!("../fixtures/meta-v6/verify-consume-tuple.scale");
		assert!(
			pallet_meta_tx::MetaTxFor::<Runtime>::decode_all(&mut meta_bytes.as_slice()).is_err()
		);
	});
}

#[cfg(any())]
#[test]
fn checked_in_meta_v6_fixtures_decode_all_and_recompute() {
	sp_io::TestExternalities::new_empty().execute_with(|| {
		frame_system::GenesisConfig::<Runtime>::default().build();
		crate::System::set_block_number(1);
		let direct_bytes = include_bytes!("../fixtures/meta-v6/direct-signed-extrinsic.scale");
		let direct = crate::UncheckedExtrinsic::decode_all(&mut direct_bytes.as_slice()).unwrap();
		assert_eq!(direct_bytes.as_slice(), direct.encode());
		let direct = direct.0;
		let sp_runtime::generic::Preamble::Signed(address, signature, extension) = direct.preamble
		else {
			panic!("direct fixture must be signed")
		};
		let crate::MultiAddress::Id(signer) = address else { panic!("fixture uses AccountId") };
		let payload = SignedPayload::new(direct.function.clone(), extension).unwrap();
		assert!(payload
			.using_encoded(|bytes| sp_runtime::traits::Verify::verify(&signature, bytes, &signer)));

		assert!(matches!(direct.function, RuntimeCall::Resources(..)));

		let meta_bytes = include_bytes!("../fixtures/meta-v6/verify-consume-tuple.scale");
		let meta =
			pallet_meta_tx::MetaTxFor::<Runtime>::decode_all(&mut meta_bytes.as_slice()).unwrap();
		assert_eq!(meta_bytes.as_slice(), meta.encode());
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
			policy,
			storage,
			metadata,
		) = extension;
		let mirror =
			crate::meta_v6::VerifySignatureMirror::decode_all(&mut verify.encode().as_slice())
				.unwrap();
		let crate::meta_v6::VerifySignatureMirror::Signed { signature, account } = mirror else {
			panic!("meta fixture must carry VerifySignature")
		};
		let bare = (
			consume, marker, nonzero, spec, tx, genesis, mortality, nonce, policy, storage,
			metadata,
		);
		let implicit = bare.implicit().unwrap();
		let digest = (version, call, bare, implicit).using_encoded(sp_io::hashing::blake2_256);
		assert!(sp_runtime::traits::Verify::verify(&signature, digest.as_slice(), &account));
		assert_fixture(
			include_bytes!("../fixtures/meta-v6/intent-preimage.scale"),
			canonical_intent(),
		);
		assert_eq!(
			include_bytes!("../fixtures/meta-v6/intent-commitment.bin").as_slice(),
			canonical_intent().commitment().as_bytes()
		);
		assert_eq!(
			include_bytes!("../fixtures/meta-v6/paid-token-key.bin").as_slice(),
			paid_token().key().as_bytes()
		);
		let max = max_envelope_with(meta);
		assert!(crate::meta_v6::inspect_paid_meta::<crate::meta_v6::ProductionMetadataImplicitResolver>(&max, 0).unwrap().is_some());
		assert_fixture(include_bytes!("../fixtures/meta-v6/max-envelope.scale"), max);
		let proof_files: [&[u8]; 7] = [
			include_bytes!("../fixtures/meta-v6/proof-person-alias.scale"),
			include_bytes!("../fixtures/meta-v6/proof-person-identity.scale"),
			include_bytes!("../fixtures/meta-v6/proof-person-alias-revised.scale"),
			include_bytes!("../fixtures/meta-v6/proof-lite-person.scale"),
			include_bytes!("../fixtures/meta-v6/proof-lite-alias.scale"),
			include_bytes!("../fixtures/meta-v6/proof-lite-alias-revised.scale"),
			include_bytes!("../fixtures/meta-v6/proof-resources-claim.scale"),
		];
		let expected = policy_vectors();
		for (index, bytes) in proof_files.into_iter().enumerate() {
			let decoded = crate::meta_v6::PolicyProofsV6::decode_all(&mut bytes.as_ref()).unwrap();
			assert_eq!(bytes, decoded.encode());
			match index {
				0 | 1 | 3 | 4 => assert_eq!(decoded, expected[index]),
				2 => assert!(matches!(decoded.personhood, Some(crate::meta_v6::MetaPersonhoodAuthV6::PersonalAliasAccountRevised(ref proof, ..)) if !proof.is_empty())),
				5 => assert!(matches!(decoded.people_lite, Some(crate::meta_v6::MetaPeopleLiteAuthV6::LiteAliasAccountRevised(ref proof, ..)) if !proof.is_empty())),
				6 => assert!(matches!(decoded.resources, Some(crate::meta_v6::MetaResourcesAuthV6::ClaimLongTermStorage(ref proof, ..)) if !proof.is_empty())),
				_ => unreachable!(),
			}
		}
		let mutation_files: [&[u8]; 11] = [
			include_bytes!("../fixtures/meta-v6/mutate-domain.scale"),
			include_bytes!("../fixtures/meta-v6/mutate-genesis.scale"),
			include_bytes!("../fixtures/meta-v6/mutate-spec.scale"),
			include_bytes!("../fixtures/meta-v6/mutate-transaction-version.scale"),
			include_bytes!("../fixtures/meta-v6/mutate-signer.scale"),
			include_bytes!("../fixtures/meta-v6/mutate-call-hash.scale"),
			include_bytes!("../fixtures/meta-v6/mutate-mortality.scale"),
			include_bytes!("../fixtures/meta-v6/mutate-nonce.scale"),
			include_bytes!("../fixtures/meta-v6/mutate-policy-hash.scale"),
			include_bytes!("../fixtures/meta-v6/mutate-storage-hash.scale"),
			include_bytes!("../fixtures/meta-v6/mutate-metadata-hash.scale"),
		];
		for (bytes, mutation) in mutation_files.into_iter().zip(mutations()) {
			assert_fixture(bytes, mutation);
		}
	});
}
