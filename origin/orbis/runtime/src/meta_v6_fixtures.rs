// This file is part of CORD – https://cord.network

// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Checked-in SCALE vectors for the Orbis v6 signed and sponsored transaction pipelines.
//!
//! These are deliberately binary fixtures. The tests decode every file with `DecodeAll` and
//! independently rebuild it from the runtime types, preventing a text fixture from silently
//! describing a wire format that the runtime does not actually use.

use crate::{AccountId, Runtime, RuntimeCall};
use codec::{DecodeAll, Encode};
use frame_support::traits::{BuildGenesisConfig, SignedTransactionBuilder};
use sp_core::{sr25519, Pair, H256};
use sp_runtime::{
	generic::{Era, SignedPayload},
	traits::{IdentifyAccount, TransactionExtension},
	MultiSignature, MultiSigner,
};

fn account(pair: &sr25519::Pair) -> AccountId {
	MultiSigner::from(pair.public()).into_account()
}

fn direct_signed_extrinsic() -> crate::UncheckedExtrinsic {
	let pair = sr25519::Pair::from_string("//Alice", None).unwrap();
	let signer = account(&pair);
	let call =
		RuntimeCall::System(frame_system::Call::remark { remark: b"orbis-v6-direct".to_vec() });
	let payment: crate::PaymentPolicy = pallet_orbis_feeless::ChargeOrSkipFeeless::from(
		pallet_asset_conversion_tx_payment::ChargeAssetTxPayment::<Runtime>::from(0, None),
	)
	.into();
	let extension = crate::paid_tx_extensions(crate::default_inner_tx_extensions(
		0,
		payment,
		Default::default(),
	));
	let payload = SignedPayload::new(call.clone(), extension.clone()).unwrap();
	let signature = payload.using_encoded(|bytes| pair.sign(bytes));
	<crate::UncheckedExtrinsic as SignedTransactionBuilder>::new_signed_transaction(
		call,
		signer.into(),
		MultiSignature::Sr25519(signature),
		extension,
	)
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
	let metadata = frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::new(false);
	let preimage = crate::meta_v6::IntentPreimageV6 {
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

fn canonical_intent() -> crate::meta_v6::IntentPreimageV6 {
	let tuple = meta_tuple(Default::default());
	let (_, _, extension): (RuntimeCall, u8, crate::MetaTxExtension) =
		DecodeAll::decode_all(&mut tuple.encode().as_slice()).unwrap();
	extension.1 .0
}

fn paid_token() -> crate::meta_v6::PaidMetaTokenV6 {
	crate::meta_v6::PaidMetaTokenV6 {
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

fn mutations() -> [crate::meta_v6::IntentPreimageV6; 11] {
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
	let mut metadata = base;
	metadata.metadata_extension_hash = [6; 32].into();
	[domain, genesis, spec, tx, signer, call, mortality, nonce, policy, storage, metadata]
}

fn assert_fixture<T: DecodeAll + Encode + PartialEq + core::fmt::Debug>(bytes: &[u8], expected: T) {
	let decoded = T::decode_all(&mut bytes.as_ref()).expect("fixture is exactly one SCALE value");
	assert_eq!(decoded, expected);
	assert_eq!(bytes, expected.encode());
}

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
		assert!(crate::meta_v6::inspect_paid_meta(&max, 0).unwrap().is_some());
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
