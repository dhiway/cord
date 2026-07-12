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
use frame_support::dispatch::GetDispatchInfo;
use frame_support::traits::BuildGenesisConfig;
use sp_core::{ed25519, sr25519, Pair, H256};
use sp_runtime::{
	generic::{Era, SignedPayload},
	traits::{IdentifyAccount, TransactionExtension},
	MultiSignature, MultiSigner,
};

const CANONICAL_METADATA_IMPLICIT: [u8; 32] = [
	0xd0, 0x3f, 0x87, 0xe6, 0x27, 0x98, 0x78, 0xca, 0xfc, 0xf6, 0x14, 0x9a, 0x71, 0x07, 0xa6, 0x30,
	0x71, 0x63, 0xe4, 0xe9, 0x97, 0xf8, 0xd7, 0x2d, 0x02, 0x9c, 0xd6, 0xaa, 0x83, 0x7f, 0x98, 0x54,
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
	crate::meta_v6::MetaAccountBoundPoliciesV6,
	pallet_bulletin_transaction_storage::extension::ValidateStorageCalls<
		Runtime,
		crate::BulletinCallInspector,
	>,
	frame_metadata_hash_extension::CheckMetadataHash<Runtime>,
);

fn meta_tuple(proofs: crate::meta_v6::PolicyProofsV6) -> pallet_meta_tx::MetaTxFor<Runtime> {
	let pair = ed25519::Pair::from_seed(&[0x42; 32]);
	let signer = MultiSigner::Ed25519(pair.public()).into_account();
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
		policy,
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
		policy,
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
			policy,
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
		policy,
		storage,
		metadata,
	) = extension;
	let mirror =
		crate::meta_v6::VerifySignatureMirror::decode_all(&mut verify.encode().as_slice()).unwrap();
	let crate::meta_v6::VerifySignatureMirror::Signed { signature, account } = mirror else {
		return false;
	};
	let bare =
		(consume, marker, nonzero, spec, tx, genesis, mortality, nonce, policy, storage, metadata);
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
		policy,
		storage,
		metadata,
	) = extension;
	let bare =
		(consume, marker, nonzero, spec, tx, genesis, mortality, nonce, policy, storage, metadata);
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
		panic!("v7 outer fixture must be signed")
	};
	let crate::MultiAddress::Id(signer) = address else { panic!("fixture uses AccountId") };
	if sponsored {
		assert!(matches!(transaction.function, RuntimeCall::MetaTx(..)));
	} else {
		assert!(matches!(transaction.function, RuntimeCall::Resources(..)));
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

#[test]
#[ignore = "run with the checked-in RUNTIME_METADATA_HASH in an isolated target"]
fn regenerate_meta_v7_fixtures() {
	use frame_support::traits::{BuildGenesisConfig, SignedTransactionBuilder};
	sp_io::TestExternalities::new_empty().execute_with(|| {
		frame_system::GenesisConfig::<Runtime>::default().build();
		crate::System::set_block_number(1);
		let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/meta-v7");
		let write = |name: &str, bytes: &[u8]| std::fs::write(dir.join(name), bytes).unwrap();
		let intent = canonical_intent();
		let token = paid_token();
		let meta = meta_tuple(Default::default());
		write("intent-preimage.scale", &intent.encode());
		write("intent-commitment.bin", intent.commitment().as_bytes());
		write("paid-token.scale", &token.encode());
		write("paid-token-key.bin", token.key().as_bytes());
		write("verify-consume-tuple.scale", &meta.encode());
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
		let legacy_bytes = include_bytes!("../fixtures/meta-v6/direct-signed-extrinsic.scale");
		let legacy = crate::UncheckedExtrinsic::decode_all(&mut legacy_bytes.as_slice()).unwrap().0;
		let sp_runtime::generic::Preamble::Signed(_, _, extension) = legacy.preamble else {
			panic!("legacy Resources fixture is signed")
		};
		assert!(matches!(legacy.function, RuntimeCall::Resources(..)));
		let direct_pair = sr25519::Pair::from_string("//Alice", None).unwrap();
		let payload = SignedPayload::new(legacy.function.clone(), extension.clone()).unwrap();
		let signature = payload.using_encoded(|bytes| direct_pair.sign(bytes));
		let signer = MultiSigner::Sr25519(direct_pair.public()).into_account();
		let direct =
			<crate::UncheckedExtrinsic as SignedTransactionBuilder>::new_signed_transaction(
				legacy.function,
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
		let (inner_call, _, extension): (RuntimeCall, u8, crate::MetaTxExtension) =
			DecodeAll::decode_all(&mut meta.encode().as_slice()).unwrap();
		assert_eq!(extension.1 .0, intent);
		assert_eq!(extension.1 .0.metadata_implicit, Some(CANONICAL_METADATA_IMPLICIT));
		assert!(extension.1.weight(&inner_call).all_gte(crate::weights::meta_v6::v7_commitment_delta()));
		let max = max_envelope_with(meta);
		assert_fixture(include_bytes!("../fixtures/meta-v7/max-envelope.scale"), max.clone());
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
			include_bytes!("../fixtures/meta-v7/mutate-metadata-implicit-none.scale"),
			include_bytes!("../fixtures/meta-v7/mutate-metadata-implicit-wrong.scale"),
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
			include_bytes!("../fixtures/meta-v7/proof-person-alias-meta.scale"),
			include_bytes!("../fixtures/meta-v7/proof-person-identity-meta.scale"),
			include_bytes!("../fixtures/meta-v7/proof-person-alias-revised-meta.scale"),
			include_bytes!("../fixtures/meta-v7/proof-lite-person-meta.scale"),
			include_bytes!("../fixtures/meta-v7/proof-lite-alias-meta.scale"),
			include_bytes!("../fixtures/meta-v7/proof-lite-alias-revised-meta.scale"),
			include_bytes!("../fixtures/meta-v7/proof-resources-claim-meta.scale"),
		];
		for (index, (bytes, expected)) in
			proof_bytes.into_iter().zip(policy_vectors()).enumerate()
		{
			let meta = pallet_meta_tx::MetaTxFor::<Runtime>::decode_all(&mut bytes.as_ref()).unwrap();
			assert_eq!(bytes, meta.encode());
			assert!(verify_meta_signature(meta.clone()));
			let (_, _, extension): (RuntimeCall, u8, crate::MetaTxExtension) =
				DecodeAll::decode_all(&mut meta.encode().as_slice()).unwrap();
			assert_eq!(extension.9 .0, expected);
			assert_eq!(extension.1 .0.metadata_implicit, Some(CANONICAL_METADATA_IMPLICIT));
			match index {
				2 => assert!(matches!(extension.9 .0.personhood, Some(crate::meta_v6::MetaPersonhoodAuthV6::PersonalAliasAccountRevised(ref proof, ..)) if !proof.is_empty())),
				5 => assert!(matches!(extension.9 .0.people_lite, Some(crate::meta_v6::MetaPeopleLiteAuthV6::LiteAliasAccountRevised(ref proof, ..)) if !proof.is_empty())),
				6 => assert!(matches!(extension.9 .0.resources, Some(crate::meta_v6::MetaResourcesAuthV6::ClaimLongTermStorage(ref proof, ..)) if !proof.is_empty())),
				_ => {},
			}
		}

		assert!(verify_meta_signature(
			pallet_meta_tx::MetaTxFor::<Runtime>::decode_all(
				&mut include_bytes!("../fixtures/meta-v7/verify-consume-tuple.scale").as_slice(),
			)
			.unwrap(),
		));
		assert!(validate_meta_head(
			pallet_meta_tx::MetaTxFor::<Runtime>::decode_all(
				&mut include_bytes!("../fixtures/meta-v7/verify-consume-tuple.scale").as_slice(),
			)
			.unwrap(),
		)
		.is_ok());
		let wrong_signature = include_bytes!("../fixtures/meta-v7/mutate-wrong-signature.scale");
		let wrong_signature =
			pallet_meta_tx::MetaTxFor::<Runtime>::decode_all(&mut wrong_signature.as_slice()).unwrap();
		assert!(!verify_meta_signature(wrong_signature.clone()));
		assert_eq!(
			validate_meta_head(wrong_signature),
			Err(sp_runtime::transaction_validity::InvalidTransaction::BadProof.into()),
		);
		assert!(pallet_meta_tx::MetaTxFor::<Runtime>::decode_all(
			&mut include_bytes!("../fixtures/meta-v7/mutate-trailing-byte.scale").as_slice(),
		)
		.is_err());
		assert_outer_fixture(
			include_bytes!("../fixtures/meta-v7/direct-signed-extrinsic.scale"),
			false,
		);
		assert_outer_fixture(
			include_bytes!("../fixtures/meta-v7/sponsored-outer-extrinsic.scale"),
			true,
		);
		assert_eq!(
			include_bytes!("../fixtures/meta-v7/intent-preimage.scale").len(),
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
			digest_hex(include_bytes!("../fixtures/meta-v7/intent-preimage.scale")),
			"e09ac3492879f4d39082570f3e08b3bb7fd5312d21532c172f5c9bf61d6b4611"
		);
		assert_eq!(crate::meta_v6::MAX_META_ENCODED_BYTES, 65_536);
		assert_eq!(crate::meta_v6::MAX_META_PAYLOAD_BYTES, 65_503);
		let manifest: serde_json::Value =
			serde_json::from_str(include_str!("../fixtures/meta-v7/manifest.json")).unwrap();
		assert_eq!(manifest["spec_version"], 28);
		assert_eq!(manifest["transaction_version"], 7);
		assert_eq!(manifest["max_envelope_bytes"], 65_536);
		assert_eq!(manifest["over_bound_error"], "InvalidTransaction::ExhaustsResources");
		assert_eq!(
			manifest["files"]
				.as_array()
				.unwrap()
				.iter()
				.filter(|row| row.get("expected_error").is_some())
				.count(),
			15,
		);
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
