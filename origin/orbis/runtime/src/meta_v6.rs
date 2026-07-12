use crate::{AccountId, Members, Runtime, RuntimeCall, RuntimeOrigin, System};
use alloc::vec::Vec;
use codec::{Decode, DecodeWithMemTracking, Encode, Output};
use frame_support::{
	traits::{Contains, OriginTrait},
	weights::Weight,
};
use indiv_support::traits::{
	Context, MembershipProver, RevisedContextualAlias, RevisionIndex, RingIndex,
};
use scale_info::TypeInfo;
use sp_core::H256;
use sp_runtime::{
	generic::{Era, ExtensionVersion},
	traits::{
		AsSystemOriginSigner, DispatchInfoOf, Implication, TransactionExtension, ValidateResult,
	},
	transaction_validity::{
		InvalidTransaction, TransactionSource, TransactionValidityError, ValidTransaction,
	},
};

pub const META_DOMAIN: &[u8] = b"orbis/meta-intent/v7";
pub const PAID_META_DOMAIN: &[u8] = b"orbis/paid-meta/v7";
pub const RESOURCES_DOMAIN: &[u8] = b"orbis/meta/v6/resources/long-term-storage";
pub const TOKEN_SLOT: &[u8] = b":orbis:paid-meta:v7";
pub const META_POLICY_INVALIDITY: u8 = 239;

#[derive(Encode, Decode, DecodeWithMemTracking, Clone, Eq, PartialEq, Debug, TypeInfo)]
pub struct IntentPreimageV7 {
	pub domain: Vec<u8>,
	pub extension_version: ExtensionVersion,
	pub genesis_hash: crate::Hash,
	pub spec_version: u32,
	pub transaction_version: u32,
	pub inner_signer: AccountId,
	pub call_hash: H256,
	pub mortality: Era,
	pub nonce: u32,
	pub policy_proofs_hash: H256,
	pub storage_extension_hash: H256,
	pub metadata_extension_hash: H256,
	pub metadata_implicit: Option<[u8; 32]>,
}

impl IntentPreimageV7 {
	pub fn commitment(&self) -> H256 {
		H256::from(sp_io::hashing::blake2_256(&self.encode()))
	}
}

#[derive(Encode, Decode, DecodeWithMemTracking, Clone, Eq, PartialEq, Debug, TypeInfo)]
pub struct PaidMetaTokenV7 {
	pub payer: AccountId,
	pub intent_commitment: H256,
	pub outer_nonce: u32,
	pub genesis_hash: crate::Hash,
	pub spec_version: u32,
	pub transaction_version: u32,
	pub consumed: bool,
}

impl PaidMetaTokenV7 {
	pub fn key(&self) -> H256 {
		H256::from(sp_io::hashing::blake2_256(&(PAID_META_DOMAIN, self).encode()))
	}
}

pub fn token() -> Option<PaidMetaTokenV7> {
	frame_support::storage::unhashed::get(TOKEN_SLOT)
}

pub fn put_token(token: &PaidMetaTokenV7) {
	frame_support::storage::unhashed::put(TOKEN_SLOT, token)
}

pub fn clear_token() {
	frame_support::storage::unhashed::kill(TOKEN_SLOT)
}

#[derive(Encode, Decode, DecodeWithMemTracking, Clone, Eq, PartialEq, Debug, TypeInfo)]
#[allow(clippy::enum_variant_names)]
pub enum MetaPersonhoodAuthV6 {
	#[codec(index = 0)]
	PersonalAliasAccount,
	#[codec(index = 1)]
	PersonalIdentityAccount,
	#[codec(index = 2)]
	PersonalAliasAccountRevised(indiv_pallet_people::types::ProofOf<Runtime>, RingIndex, Context),
}

#[derive(Encode, Decode, DecodeWithMemTracking, Clone, Eq, PartialEq, Debug, TypeInfo)]
#[allow(clippy::enum_variant_names)]
pub enum MetaPeopleLiteAuthV6 {
	#[codec(index = 0)]
	LitePerson,
	#[codec(index = 1)]
	LiteAliasAccount,
	#[codec(index = 2)]
	LiteAliasAccountRevised(indiv_pallet_people_lite::types::ProofOf<Runtime>, RingIndex, Context),
}

#[derive(Encode, Decode, DecodeWithMemTracking, Clone, Eq, PartialEq, Debug, TypeInfo)]
pub enum MetaResourcesAuthV6 {
	#[codec(index = 0)]
	ClaimLongTermStorage(
		indiv_pallet_resources::types::ProofOf<Runtime>,
		RingIndex,
		RevisionIndex,
		indiv_pallet_resources::types::MembershipCollection,
	),
}

#[derive(Encode, Decode, DecodeWithMemTracking, Clone, Eq, PartialEq, Debug, TypeInfo, Default)]
pub struct PolicyProofsV6 {
	pub personhood: Option<MetaPersonhoodAuthV6>,
	pub people_lite: Option<MetaPeopleLiteAuthV6>,
	pub resources: Option<MetaResourcesAuthV6>,
}

#[derive(Encode, Decode, DecodeWithMemTracking, Clone, Eq, PartialEq, Debug, TypeInfo, Default)]
pub struct MetaAccountBoundPoliciesV6(pub PolicyProofsV6);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RouterRouteV6 {
	None,
	PersonalAlias,
	PersonalIdentity,
	PersonalAliasRevised,
	LitePerson,
	LiteAlias,
	LiteAliasRevised,
	ResourcesClaim,
}

impl MetaAccountBoundPoliciesV6 {
	pub fn new(proofs: PolicyProofsV6) -> Self {
		Self(proofs)
	}

	/// Classify the call/proof pair before touching membership storage or verifying a proof.
	fn classify(&self, call: &RuntimeCall) -> Result<RouterRouteV6, InvalidTransaction> {
		if self.0.personhood.is_some() as u8
			+ self.0.people_lite.is_some() as u8
			+ self.0.resources.is_some() as u8
			> 1
		{
			return Err(InvalidTransaction::Call);
		}
		let resources_call = matches!(
			call,
			RuntimeCall::Resources(indiv_pallet_resources::Call::claim_long_term_storage { .. })
		);
		match (&self.0.personhood, &self.0.people_lite, &self.0.resources) {
			(None, None, Some(MetaResourcesAuthV6::ClaimLongTermStorage(..))) if resources_call => {
				Ok(RouterRouteV6::ResourcesClaim)
			},
			(_, _, _) if resources_call => Err(InvalidTransaction::Call),
			(Some(MetaPersonhoodAuthV6::PersonalAliasAccount), None, None) => {
				Ok(RouterRouteV6::PersonalAlias)
			},
			(Some(MetaPersonhoodAuthV6::PersonalIdentityAccount), None, None) => {
				Ok(RouterRouteV6::PersonalIdentity)
			},
			(Some(MetaPersonhoodAuthV6::PersonalAliasAccountRevised(..)), None, None) => {
				Ok(RouterRouteV6::PersonalAliasRevised)
			},
			(None, Some(MetaPeopleLiteAuthV6::LitePerson), None) => Ok(RouterRouteV6::LitePerson),
			(None, Some(MetaPeopleLiteAuthV6::LiteAliasAccount), None) => {
				Ok(RouterRouteV6::LiteAlias)
			},
			(None, Some(MetaPeopleLiteAuthV6::LiteAliasAccountRevised(..)), None) => {
				Ok(RouterRouteV6::LiteAliasRevised)
			},
			(None, None, None) => Ok(RouterRouteV6::None),
			_ => Err(InvalidTransaction::Call),
		}
	}
}

/// Execute the production policy classifier/weight selector for FRAME's Resources-hosted Meta
/// benchmarks. Dummy proof values are decoded only to reach the same production variant and are
/// never accepted as authorization; proof verification itself is covered by the pallet's existing
/// ring-proof benchmarks. Keeping this adapter here prevents a benchmark-only copy of the router
/// from drifting from `MetaAccountBoundPoliciesV6`.
#[cfg(feature = "runtime-benchmarks")]
pub fn benchmark_policy_scenario(
	scenario: indiv_pallet_resources::benchmarking::MetaPolicyBenchmarkScenario,
) -> Result<(), frame_benchmarking::BenchmarkError> {
	use codec::DecodeAll;
	use frame_support::dispatch::GetDispatchInfo;
	use indiv_pallet_resources::benchmarking::MetaPolicyBenchmarkScenario as Scenario;
	use indiv_support::traits::{AppendOnlyMembers, RingMode};
	use sp_runtime::{
		traits::{TrailingZeroInput, TransactionExtension},
		transaction_validity::{InvalidTransaction, TransactionSource, TransactionValidityError},
	};
	use verifiable::GenerateVerifiable;

	type Crypto = verifiable::ring::bandersnatch::BandersnatchVrfVerifiable;
	type BenchmarkSecret = <Crypto as GenerateVerifiable>::Secret;
	type BenchmarkCommitment = <Crypto as GenerateVerifiable>::Commitment;
	fn stop(error: impl core::fmt::Debug) -> frame_benchmarking::BenchmarkError {
		log::error!(target: "orbis-meta-benchmark", "{error:?}");
		frame_benchmarking::BenchmarkError::Stop("production Meta benchmark workload failed")
	}
	struct MissingMetadata;
	impl MetadataImplicitResolver for MissingMetadata {
		fn resolve(
			_: &frame_metadata_hash_extension::CheckMetadataHash<Runtime>,
		) -> Result<Option<[u8; 32]>, TransactionValidityError> {
			Err(sp_runtime::transaction_validity::UnknownTransaction::CannotLookup.into())
		}
	}
	#[derive(Clone, Copy, Eq, PartialEq)]
	struct BenchmarkCompiledMetadata;
	impl MetadataImplicitResolver for BenchmarkCompiledMetadata {
		fn resolve(
			metadata: &frame_metadata_hash_extension::CheckMetadataHash<Runtime>,
		) -> Result<Option<[u8; 32]>, TransactionValidityError> {
			if metadata.encode() == [0] {
				Ok(None)
			} else {
				Ok(Some([0xd0; 32]))
			}
		}
	}
	fn benchmark_outer(enabled: bool) -> Result<RuntimeCall, frame_benchmarking::BenchmarkError> {
		let bytes = include_bytes!("../fixtures/meta-v7/verify-consume-tuple.scale");
		let meta = pallet_meta_tx::MetaTxFor::<Runtime>::decode_all(&mut bytes.as_slice())
			.map_err(stop)?;
		let (call, version, extension): (RuntimeCall, u8, crate::MetaTxExtension) =
			DecodeAll::decode_all(&mut meta.encode().as_slice()).map_err(stop)?;
		let (
			verify,
			mut consume,
			marker,
			nonzero,
			spec,
			tx,
			genesis,
			mortality,
			nonce,
			identity_policies,
			storage,
			old_metadata,
		) = extension;
		let (score, policy, honour) = identity_policies;
		let metadata = if enabled {
			old_metadata
		} else {
			frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::decode_all(&mut &[0u8][..])
				.map_err(stop)?
		};
		let VerifySignatureMirror::Signed { account, .. } =
			VerifySignatureMirror::decode_all(&mut verify.encode().as_slice()).map_err(stop)?
		else {
			return Err(stop("benchmark fixture signature disabled"));
		};
		consume.0.domain = META_DOMAIN.to_vec();
		consume.0.extension_version = version;
		consume.0.genesis_hash = System::block_hash(0);
		consume.0.spec_version = crate::VERSION.spec_version;
		consume.0.transaction_version = crate::VERSION.transaction_version;
		consume.0.inner_signer = account;
		consume.0.call_hash = hash_encoded(&call);
		consume.0.mortality = mortality.0;
		consume.0.nonce = nonce.0;
		consume.0.policy_proofs_hash = hash_encoded(&policy.0);
		consume.0.storage_extension_hash = hash_encoded(&storage);
		consume.0.metadata_extension_hash = hash_encoded(&metadata);
		consume.0.metadata_implicit = if enabled { Some([0xd0; 32]) } else { None };
		let rebuilt = pallet_meta_tx::MetaTxFor::<Runtime>::new(
			call,
			version,
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
				(score, policy, honour),
				storage,
				metadata,
			),
		);
		let len = rebuilt.encoded_size() as u32;
		Ok(RuntimeCall::MetaTx(pallet_meta_tx::Call::dispatch {
			meta_tx: alloc::boxed::Box::new(rebuilt),
			meta_tx_encoded_len: len,
		}))
	}
	fn consume_benchmark_token(
		outer: &RuntimeCall,
		commitment: H256,
	) -> Result<(), frame_benchmarking::BenchmarkError> {
		let RuntimeCall::MetaTx(pallet_meta_tx::Call::dispatch { meta_tx, .. }) = outer else {
			return Err(stop("benchmark outer is not MetaTx"));
		};
		let (call, _, extension): (RuntimeCall, u8, crate::MetaTxExtension) =
			DecodeAll::decode_all(&mut meta_tx.encode().as_slice()).map_err(stop)?;
		let consume = extension.1;
		put_token(&PaidMetaTokenV7 {
			payer: AccountId::new([9; 32]),
			intent_commitment: commitment,
			outer_nonce: 0,
			genesis_hash: System::block_hash(0),
			spec_version: crate::VERSION.spec_version,
			transaction_version: crate::VERSION.transaction_version,
			consumed: false,
		});
		let origin = RuntimeOrigin::signed(consume.0.inner_signer.clone());
		let (_, key, origin) = consume
			.validate(
				origin,
				&call,
				&call.get_dispatch_info(),
				call.encoded_size(),
				(),
				&sp_runtime::traits::TxBaseImplication((0u8, &call)),
				TransactionSource::External,
			)
			.map_err(stop)?;
		consume
			.prepare(key, &origin, &call, &call.get_dispatch_info(), call.encoded_size())
			.map_err(stop)?;
		if token().is_some() {
			return Err(stop("Consume did not clear exact benchmark token"));
		}
		Ok(())
	}
	match scenario {
		Scenario::MetadataEnabled => {
			let outer = benchmark_outer(true)?;
			let commitment = inspect_paid_meta::<BenchmarkCompiledMetadata>(&outer, 0)
				.map_err(stop)?
				.ok_or_else(|| stop("enabled Meta not inspected"))?;
			consume_benchmark_token(&outer, commitment)?;
			return Ok(());
		},
		Scenario::MetadataDisabled => {
			let outer = benchmark_outer(false)?;
			let commitment = inspect_paid_meta::<BenchmarkCompiledMetadata>(&outer, 0)
				.map_err(stop)?
				.ok_or_else(|| stop("disabled Meta not inspected"))?;
			consume_benchmark_token(&outer, commitment)?;
			return Ok(());
		},
		Scenario::MetadataCannotLookup => {
			let outer = benchmark_outer(true)?;
			if !matches!(
				inspect_paid_meta::<MissingMetadata>(&outer, 0),
				Err(TransactionValidityError::Unknown(
					sp_runtime::transaction_validity::UnknownTransaction::CannotLookup
				))
			) {
				return Err(stop("missing metadata unexpectedly resolved"));
			}
			return Ok(());
		},
		Scenario::MetadataMax => {
			let outer = benchmark_outer(true)?;
			let make = |padding: usize| {
				RuntimeCall::Utility(pallet_utility::Call::batch {
					calls: alloc::vec![
						outer.clone(),
						RuntimeCall::System(frame_system::Call::remark {
							remark: alloc::vec![0; padding],
						}),
					],
				})
			};
			let empty = make(0).encoded_size();
			let target = MAX_META_ENCODED_BYTES.saturating_sub(empty);
			let accepted = (target.saturating_sub(16)..=target.saturating_add(16))
				.map(make)
				.find(|call| call.encoded_size() == MAX_META_ENCODED_BYTES)
				.ok_or_else(|| stop("could not build exact max envelope"))?;
			inspect_paid_meta::<BenchmarkCompiledMetadata>(&accepted, 0)
				.map_err(stop)?
				.ok_or_else(|| stop("max envelope lost Meta"))?;
			let RuntimeCall::Utility(pallet_utility::Call::batch { mut calls }) = accepted else {
				unreachable!()
			};
			let RuntimeCall::System(frame_system::Call::remark { remark }) = &mut calls[1] else {
				unreachable!()
			};
			remark.push(0);
			let over = RuntimeCall::Utility(pallet_utility::Call::batch { calls });
			if !matches!(
				inspect_paid_meta::<BenchmarkCompiledMetadata>(&over, 0),
				Err(TransactionValidityError::Invalid(InvalidTransaction::ExhaustsResources))
			) {
				return Err(stop("over-bound envelope was not rejected"));
			}
			return Ok(());
		},
		_ => {},
	}
	fn install_ring(
		identifier: [u8; 32],
		seed: u8,
	) -> Result<(BenchmarkSecret, BenchmarkCommitment, u32), frame_benchmarking::BenchmarkError> {
		let exponent = crate::MembersFlexibleRingExponent::get();
		let domain: verifiable::ring::RingDomainSize = exponent.try_into().map_err(stop)?;
		let chunks = indiv_support::genesis::ring_verifier_builder_params::<
			verifiable::ring::ark_vrf::suites::bandersnatch::BandersnatchSha512Ell2,
		>(domain);
		for (page_index, page) in
			chunks.chunks(crate::PeopleChunkPageSize::get() as usize).enumerate()
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
				.map_err(stop)?;
			indiv_pallet_chunks_manager::Chunks::<Runtime>::insert(
				exponent,
				page_index as u32,
				page,
			);
		}
		<Members as AppendOnlyMembers>::create_collection(
			crate::Location::here(),
			&identifier,
			1,
			RingMode::Flexible,
			exponent,
			None,
		)
		.map_err(stop)?;
		let secret = Crypto::new_secret([seed; 32]);
		let member = Crypto::member_from_secret(&secret);
		<Members as AppendOnlyMembers>::add_members(&identifier, alloc::vec![member.clone()])
			.map_err(stop)?;
		Members::onboard_members_authorized(
			frame_system::RawOrigin::Authorized.into(),
			identifier,
			0,
			0,
			Some(member.clone()),
			0,
		)
		.map_err(stop)?;
		Members::build_ring_authorized(
			frame_system::RawOrigin::Authorized.into(),
			identifier,
			0,
			exponent,
			None,
			1,
			0,
		)
		.map_err(stop)?;
		let revision = <Members as MembershipProver>::ring_revision(&identifier, 0)
			.ok_or_else(|| stop("missing revision"))?;
		let commitment = Crypto::open(
			exponent.try_into().map_err(stop)?,
			&member,
			<Members as AppendOnlyMembers>::ring_members(&identifier, 0).into_iter(),
		)
		.map_err(stop)?;
		Ok((secret, commitment, revision))
	}
	fn validate_prepare(
		policy: MetaAccountBoundPoliciesV6,
		signer: AccountId,
		call: &RuntimeCall,
	) -> Result<(), frame_benchmarking::BenchmarkError> {
		let info = call.get_dispatch_info();
		let implication = sp_runtime::traits::TxBaseImplication((0u8, call));
		let implicit = policy.implicit().map_err(stop)?;
		let (_, val, origin) = policy
			.validate(
				RuntimeOrigin::signed(signer),
				call,
				&info,
				call.encoded_size(),
				implicit,
				&implication,
				TransactionSource::External,
			)
			.map_err(stop)?;
		policy.prepare(val, &origin, call, &info, call.encoded_size()).map_err(stop)?;
		Ok(())
	}

	let signer = AccountId::decode(&mut TrailingZeroInput::zeroes()).map_err(stop)?;
	let ordinary = RuntimeCall::System(frame_system::Call::remark { remark: Vec::new() });
	let people_id = *indiv_pallet_people::PEOPLE_MEMBER_IDENTIFIER;
	let lite_id = *indiv_pallet_people_lite::LITE_PEOPLE_MEMBER_IDENTIFIER;
	let policies = match scenario {
		Scenario::PersonalAlias | Scenario::MappingMiss => PolicyProofsV6 {
			personhood: Some(MetaPersonhoodAuthV6::PersonalAliasAccount),
			..Default::default()
		},
		Scenario::PersonalIdentity => PolicyProofsV6 {
			personhood: Some(MetaPersonhoodAuthV6::PersonalIdentityAccount),
			..Default::default()
		},
		Scenario::PersonalAliasRevised | Scenario::RevisedWrite => {
			let (secret, old_commitment, old_revision) = install_ring(people_id, 41)?;
			let (_, alias) =
				Crypto::create(old_commitment, &secret, &crate::ORBIS_PERSON_CONTEXT, &[0; 32])
					.map_err(stop)?;
			let old = RevisedContextualAlias {
				revision: old_revision,
				ring: 0,
				ca: indiv_support::traits::ContextualAlias {
					context: crate::ORBIS_PERSON_CONTEXT,
					alias,
				},
			};
			indiv_pallet_people::AccountToAlias::<Runtime>::insert(&signer, &old);
			indiv_pallet_people::AliasToAccount::<Runtime>::insert(&old.ca, &signer);
			let second = Crypto::member_from_secret(&Crypto::new_secret([42; 32]));
			<Members as AppendOnlyMembers>::add_members(&people_id, alloc::vec![second.clone()])
				.map_err(stop)?;
			Members::onboard_members_authorized(
				frame_system::RawOrigin::Authorized.into(),
				people_id,
				0,
				1,
				Some(second),
				0,
			)
			.map_err(stop)?;
			Members::build_ring_authorized(
				frame_system::RawOrigin::Authorized.into(),
				people_id,
				0,
				crate::MembersFlexibleRingExponent::get(),
				Some(old_revision),
				1,
				1,
			)
			.map_err(stop)?;
			let member = Crypto::member_from_secret(&secret);
			let commitment = Crypto::open(
				crate::MembersFlexibleRingExponent::get().try_into().map_err(stop)?,
				&member,
				<Members as AppendOnlyMembers>::ring_members(&people_id, 0).into_iter(),
			)
			.map_err(stop)?;
			let implication = sp_runtime::traits::TxBaseImplication((0u8, &ordinary));
			let msg = (
				b"orbis/meta/v6/personhood/alias-revised",
				&signer,
				&signer,
				&ordinary,
				implication,
			)
				.using_encoded(sp_io::hashing::blake2_256);
			let (proof, _) =
				Crypto::create(commitment, &secret, &crate::ORBIS_PERSON_CONTEXT, &msg)
					.map_err(stop)?;
			PolicyProofsV6 {
				personhood: Some(MetaPersonhoodAuthV6::PersonalAliasAccountRevised(
					proof,
					0,
					crate::ORBIS_PERSON_CONTEXT,
				)),
				..Default::default()
			}
		},
		Scenario::LitePerson => PolicyProofsV6 {
			people_lite: Some(MetaPeopleLiteAuthV6::LitePerson),
			..Default::default()
		},
		Scenario::LiteAlias => PolicyProofsV6 {
			people_lite: Some(MetaPeopleLiteAuthV6::LiteAliasAccount),
			..Default::default()
		},
		Scenario::LiteAliasRevised => {
			let (secret, old_commitment, old_revision) = install_ring(lite_id, 51)?;
			let context = *indiv_pallet_people_lite::LITE_PEOPLE_AUTH_CONTEXT;
			let (_, alias) =
				Crypto::create(old_commitment, &secret, &context, &[0; 32]).map_err(stop)?;
			let old = RevisedContextualAlias {
				revision: old_revision,
				ring: 0,
				ca: indiv_support::traits::ContextualAlias { context, alias },
			};
			indiv_pallet_people_lite::AccountToAlias::<Runtime>::insert(&signer, &old);
			indiv_pallet_people_lite::AliasToAccount::<Runtime>::insert(&old.ca, &signer);
			let second = Crypto::member_from_secret(&Crypto::new_secret([52; 32]));
			<Members as AppendOnlyMembers>::add_members(&lite_id, alloc::vec![second.clone()])
				.map_err(stop)?;
			Members::onboard_members_authorized(
				frame_system::RawOrigin::Authorized.into(),
				lite_id,
				0,
				1,
				Some(second),
				0,
			)
			.map_err(stop)?;
			Members::build_ring_authorized(
				frame_system::RawOrigin::Authorized.into(),
				lite_id,
				0,
				crate::MembersFlexibleRingExponent::get(),
				Some(old_revision),
				1,
				1,
			)
			.map_err(stop)?;
			let member = Crypto::member_from_secret(&secret);
			let commitment = Crypto::open(
				crate::MembersFlexibleRingExponent::get().try_into().map_err(stop)?,
				&member,
				<Members as AppendOnlyMembers>::ring_members(&lite_id, 0).into_iter(),
			)
			.map_err(stop)?;
			let implication = sp_runtime::traits::TxBaseImplication((0u8, &ordinary));
			let msg = (
				b"orbis/meta/v6/people-lite/alias-revised",
				&signer,
				&signer,
				&ordinary,
				implication,
			)
				.using_encoded(sp_io::hashing::blake2_256);
			let (proof, _) = Crypto::create(commitment, &secret, &context, &msg).map_err(stop)?;
			PolicyProofsV6 {
				people_lite: Some(MetaPeopleLiteAuthV6::LiteAliasAccountRevised(proof, 0, context)),
				..Default::default()
			}
		},
		Scenario::ResourcesClaim | Scenario::MaxProof => {
			pallet_timestamp::Now::<Runtime>::put(3 * 24 * 60 * 60 * 1_000u64);
			let (secret, commitment, revision) = install_ring(people_id, 61)?;
			let period = crate::Resources::long_term_storage_period_from_timestamp(
				<crate::Timestamp as frame_support::traits::UnixTime>::now().as_secs(),
			);
			let call =
				RuntimeCall::Resources(indiv_pallet_resources::Call::claim_long_term_storage {
					period,
					counter: 0,
					account_id: signer.clone(),
				});
			let context = crate::Resources::long_term_storage_context(period, 0);
			let implication = sp_runtime::traits::TxBaseImplication((0u8, &call));
			let (_, alias) =
				Crypto::create(commitment.clone(), &secret, &context, &[0; 32]).map_err(stop)?;
			let bound = RevisedContextualAlias {
				revision,
				ring: 0,
				ca: indiv_support::traits::ContextualAlias {
					context: crate::ORBIS_PERSON_CONTEXT,
					alias,
				},
			};
			indiv_pallet_people::AccountToAlias::<Runtime>::insert(&signer, &bound);
			indiv_pallet_people::AliasToAccount::<Runtime>::insert(&bound.ca, &signer);
			let msg = (
				RESOURCES_DOMAIN,
				&signer,
				&signer,
				alias,
				period,
				0u8,
				indiv_pallet_resources::types::MembershipCollection::People,
				0u32,
				revision,
				context,
				&call,
				implication,
			)
				.using_encoded(sp_io::hashing::blake2_256);
			let (proof, verified_alias) = if matches!(scenario, Scenario::MaxProof) {
				let contexts = [&context[..]; 16];
				let (proof, aliases) =
					Crypto::create_multi_context(commitment, &secret, &contexts, &msg)
						.map_err(stop)?;
				if proof.encoded_size() < 1_265 {
					return Err(stop("maximum proof was not generated"));
				}
				(proof, aliases[0])
			} else {
				Crypto::create(commitment, &secret, &context, &msg).map_err(stop)?
			};
			if verified_alias != alias {
				return Err(stop("resource proof alias drift"));
			}
			let policy = MetaAccountBoundPoliciesV6::new(PolicyProofsV6 {
				resources: Some(MetaResourcesAuthV6::ClaimLongTermStorage(
					proof,
					0,
					revision,
					indiv_pallet_resources::types::MembershipCollection::People,
				)),
				..Default::default()
			});
			if matches!(scenario, Scenario::MaxProof) {
				let encoded = policy.encode();
				if encoded.len() > MAX_META_ENCODED_BYTES {
					return Err(stop("proof exceeds envelope bound"));
				}
				core::hint::black_box(encoded);
				let info = call.get_dispatch_info();
				let rejected = policy.validate(
					RuntimeOrigin::signed(signer),
					&call,
					&info,
					call.encoded_size(),
					(),
					&sp_runtime::traits::TxBaseImplication((0u8, &call)),
					TransactionSource::External,
				);
				if !matches!(
					rejected,
					Err(TransactionValidityError::Invalid(InvalidTransaction::BadProof))
				) {
					return Err(stop("maximum multi-context proof was not rejected"));
				}
				return Ok(());
			}
			validate_prepare(policy, signer, &call)?;
			return Ok(());
		},
		Scenario::Malformed => PolicyProofsV6 {
			personhood: Some(MetaPersonhoodAuthV6::PersonalAliasAccount),
			people_lite: Some(MetaPeopleLiteAuthV6::LitePerson),
			resources: None,
		},
		Scenario::Envelope => {
			let calls = (0..MAX_META_ENVELOPE_CALLS.saturating_sub(1))
				.map(|_| ordinary.clone())
				.collect();
			let envelope = RuntimeCall::Utility(pallet_utility::Call::batch { calls });
			let scope = PaidMetaScope::new(frame_system::CheckSpecVersion::<Runtime>::new());
			let info = envelope.get_dispatch_info();
			let implicit = scope.implicit().map_err(stop)?;
			let (_, val, origin) = scope
				.validate(
					RuntimeOrigin::signed(signer.clone()),
					&envelope,
					&info,
					envelope.encoded_size(),
					implicit,
					&sp_runtime::traits::TxBaseImplication((0u8, &envelope)),
					TransactionSource::External,
				)
				.map_err(stop)?;
			scope
				.prepare(val, &origin, &envelope, &info, envelope.encoded_size())
				.map_err(stop)?;
			// Benchmark the production VerifySignature mirror decode as well as the bounded
			// PaidMetaScope tree walk. Under `runtime-benchmarks` pallet-meta-tx substitutes its
			// weightless extension, so the production tuple is intentionally inspected as its exact
			// SCALE payload rather than wrapped in the benchmark-only RuntimeCall variant.
			let mortality = frame_system::CheckMortality::<Runtime>::from(Era::Immortal);
			let nonce = frame_system::CheckNonce::<Runtime>::from(0);
			let policy = MetaAccountBoundPoliciesV6::default();
			let storage = pallet_bulletin_transaction_storage::extension::ValidateStorageCalls::<
				Runtime,
				crate::BulletinCallInspector,
			>::default();
			let metadata = crate::canonical_metadata_extension();
			let metadata_implicit =
				bulletin_pallets_common::resolve_metadata_implicit::<RuntimeCall, _>(&metadata)
					.map_err(|_| stop("metadata implicit unavailable"))?;
			let preimage = IntentPreimageV7 {
				domain: META_DOMAIN.to_vec(),
				extension_version: 0,
				genesis_hash: System::block_hash(0),
				spec_version: crate::VERSION.spec_version,
				transaction_version: crate::VERSION.transaction_version,
				inner_signer: signer.clone(),
				call_hash: hash_encoded(&ordinary),
				mortality: Era::Immortal,
				nonce: 0,
				policy_proofs_hash: hash_encoded(&policy.0),
				storage_extension_hash: hash_encoded(&storage),
				metadata_extension_hash: hash_encoded(&metadata),
				metadata_implicit,
			};
			let verify = pallet_verify_signature::VerifySignature::new_with_signature(
				sp_runtime::MultiSignature::Ed25519(sp_core::ed25519::Signature::from_raw([0; 64])),
				signer,
			);
			let extension: crate::MetaTxExtension = (
				verify,
				ConsumePaidMetaIngress(preimage),
				pallet_meta_tx::MetaTxMarker::new(),
				frame_system::CheckNonZeroSender::new(),
				frame_system::CheckSpecVersion::new(),
				frame_system::CheckTxVersion::new(),
				frame_system::CheckGenesis::new(),
				mortality,
				nonce,
				(
					pallet_orbis_score::ScoreAsParticipant::<Runtime>::new(None),
					policy,
					pallet_orbis_honour::extension::VoterAuth::<Runtime>::new(None),
				),
				storage,
				metadata,
			);
			let payload = (ordinary, 0u8, extension).encode();
			decode_meta_payload::<ProductionMetadataImplicitResolver>(&payload).map_err(stop)?;
			return Ok(());
		},
		Scenario::MetadataEnabled
		| Scenario::MetadataDisabled
		| Scenario::MetadataCannotLookup
		| Scenario::MetadataMax => unreachable!("metadata scenarios return before policy setup"),
	};
	if matches!(scenario, Scenario::Malformed) {
		let policy = MetaAccountBoundPoliciesV6::new(policies);
		let info = ordinary.get_dispatch_info();
		let result = policy.validate(
			RuntimeOrigin::signed(signer),
			&ordinary,
			&info,
			ordinary.encoded_size(),
			(),
			&sp_runtime::traits::TxBaseImplication((0u8, &ordinary)),
			TransactionSource::External,
		);
		if !matches!(result, Err(TransactionValidityError::Invalid(InvalidTransaction::Call))) {
			return Err(stop("malformed accepted"));
		}
		return Ok(());
	}
	if matches!(scenario, Scenario::MappingMiss) {
		let policy = MetaAccountBoundPoliciesV6::new(policies);
		let info = ordinary.get_dispatch_info();
		let result = policy.validate(
			RuntimeOrigin::signed(signer),
			&ordinary,
			&info,
			ordinary.encoded_size(),
			(),
			&sp_runtime::traits::TxBaseImplication((0u8, &ordinary)),
			TransactionSource::External,
		);
		if !matches!(result, Err(TransactionValidityError::Invalid(InvalidTransaction::BadSigner)))
		{
			return Err(stop("mapping miss accepted"));
		}
		return Ok(());
	}
	let call = &ordinary;
	let policy = MetaAccountBoundPoliciesV6::new(policies);
	// Populate the authoritative production mappings used by the four non-proof routes.
	match scenario {
		Scenario::PersonalAlias => {
			let (secret, commitment, revision) = install_ring(people_id, 31)?;
			let (_, alias) =
				Crypto::create(commitment, &secret, &crate::ORBIS_PERSON_CONTEXT, &[0; 32])
					.map_err(stop)?;
			let bound = RevisedContextualAlias {
				revision,
				ring: 0,
				ca: indiv_support::traits::ContextualAlias {
					context: crate::ORBIS_PERSON_CONTEXT,
					alias,
				},
			};
			indiv_pallet_people::AccountToAlias::<Runtime>::insert(&signer, &bound);
			indiv_pallet_people::AliasToAccount::<Runtime>::insert(&bound.ca, &signer);
		},
		Scenario::PersonalIdentity => {
			let member = Crypto::member_from_secret(&Crypto::new_secret([32; 32]));
			indiv_pallet_people::AccountToPersonalId::<Runtime>::insert(&signer, 0);
			indiv_pallet_people::People::<Runtime>::insert(
				0,
				indiv_pallet_people::types::PersonRecord {
					key: member,
					account: Some(signer.clone()),
				},
			);
		},
		Scenario::LitePerson => {
			let member = Crypto::member_from_secret(&Crypto::new_secret([33; 32]));
			indiv_pallet_people_lite::LitePeople::<Runtime>::insert(
				&signer,
				indiv_pallet_people_lite::types::LitePersonInfo {
					ring_vrf_key: member,
					method: indiv_pallet_people_lite::types::RecognitionMethod::UniqueDevice(
						signer.clone(),
					),
				},
			);
		},
		Scenario::LiteAlias => {
			let (secret, commitment, revision) = install_ring(lite_id, 34)?;
			let context = *indiv_pallet_people_lite::LITE_PEOPLE_AUTH_CONTEXT;
			let (_, alias) =
				Crypto::create(commitment, &secret, &context, &[0; 32]).map_err(stop)?;
			let bound = RevisedContextualAlias {
				revision,
				ring: 0,
				ca: indiv_support::traits::ContextualAlias { context, alias },
			};
			indiv_pallet_people_lite::AccountToAlias::<Runtime>::insert(&signer, &bound);
			indiv_pallet_people_lite::AliasToAccount::<Runtime>::insert(&bound.ca, &signer);
		},
		_ => {},
	}
	validate_prepare(policy, signer.clone(), call)?;
	if matches!(scenario, Scenario::RevisedWrite)
		&& !indiv_pallet_people::AccountToAlias::<Runtime>::get(&signer)
			.is_some_and(|binding| binding.revision > 0)
	{
		return Err(stop("revised binding was not written"));
	}
	Ok(())
}

pub enum PolicyValV6 {
	None,
	PersonalAlias(AccountId),
	PersonalIdentity(AccountId),
	LitePerson(AccountId),
	LiteAlias(AccountId),
	ResourcesClaim(AccountId, indiv_support::traits::Alias),
	PersonRevision(AccountId, RevisedContextualAlias),
	LiteRevision(AccountId, RevisedContextualAlias),
}

pub enum PolicyPreV6 {
	None,
	PersonalAlias(AccountId),
	PersonalIdentity(AccountId),
	LitePerson(AccountId),
	LiteAlias(AccountId),
	ResourcesClaim(AccountId, indiv_support::traits::Alias),
	PersonRevision(AccountId, RevisionIndex),
	LiteRevision(AccountId, RevisionIndex),
}

impl TransactionExtension<RuntimeCall> for MetaAccountBoundPoliciesV6 {
	const IDENTIFIER: &'static str = "MetaAccountBoundPoliciesV6";
	type Implicit = ();
	type Val = PolicyValV6;
	type Pre = PolicyPreV6;

	fn weight(&self, call: &RuntimeCall) -> Weight {
		use indiv_pallet_resources::weights::WeightInfo as _;
		let resources_weight = if matches!(
			call,
			RuntimeCall::Resources(indiv_pallet_resources::Call::claim_long_term_storage { .. })
		) {
			<Runtime as indiv_pallet_resources::Config>::WeightInfo::claim_long_term_storage_tx_ext(
			)
		} else {
			Weight::zero()
		};
		let route_weight = match self.classify(call) {
			Ok(RouterRouteV6::None) => crate::weights::meta_v6::none(),
			Ok(RouterRouteV6::PersonalAlias) =>
				<Runtime as indiv_pallet_resources::Config>::WeightInfo::meta_policy_personal_alias(),
			Ok(RouterRouteV6::PersonalIdentity) => <Runtime as indiv_pallet_resources::Config>::WeightInfo::meta_policy_personal_identity(),
			Ok(RouterRouteV6::PersonalAliasRevised) => <Runtime as indiv_pallet_resources::Config>::WeightInfo::meta_policy_personal_alias_revised(),
			Ok(RouterRouteV6::LitePerson) =>
				<Runtime as indiv_pallet_resources::Config>::WeightInfo::meta_policy_lite_person(),
			Ok(RouterRouteV6::LiteAlias) =>
				<Runtime as indiv_pallet_resources::Config>::WeightInfo::meta_policy_lite_alias(),
			Ok(RouterRouteV6::LiteAliasRevised) => <Runtime as indiv_pallet_resources::Config>::WeightInfo::meta_policy_lite_alias_revised(),
			Ok(RouterRouteV6::ResourcesClaim) => <Runtime as indiv_pallet_resources::Config>::WeightInfo::meta_policy_resources_claim(),
			Err(_) => <Runtime as indiv_pallet_resources::Config>::WeightInfo::meta_policy_malformed(),
		};
		resources_weight.saturating_add(route_weight)
	}

	fn validate(
		&self,
		mut origin: RuntimeOrigin,
		call: &RuntimeCall,
		_info: &DispatchInfoOf<RuntimeCall>,
		_len: usize,
		_: (),
		inherited: &impl Implication,
		_source: TransactionSource,
	) -> ValidateResult<PolicyValV6, RuntimeCall> {
		// Route before doing any People/Lite lookup or proof verification.
		let route = self.classify(call).map_err(TransactionValidityError::Invalid)?;
		let signer = origin
			.as_system_origin_signer()
			.cloned()
			.or_else(|| match frame_support::traits::OriginTrait::caller(&origin) {
				crate::OriginCaller::Score(
					pallet_orbis_score::Origin::<Runtime>::AccountParticipant(account),
				) => Some(account.clone()),
				_ => None,
			})
			.ok_or(InvalidTransaction::BadSigner)?;
		if let Some(personhood) = &self.0.personhood {
			debug_assert!(matches!(
				route,
				RouterRouteV6::PersonalAlias
					| RouterRouteV6::PersonalIdentity
					| RouterRouteV6::PersonalAliasRevised
			));
			let (local, value) = match personhood {
				MetaPersonhoodAuthV6::PersonalAliasAccount => {
					let bound = indiv_pallet_people::AccountToAlias::<Runtime>::get(&signer)
						.ok_or(InvalidTransaction::BadSigner)?;
					if !<Runtime as indiv_pallet_people::Config>::AccountContexts::contains(
						&bound.ca.context,
					) || indiv_pallet_people::AliasToAccount::<Runtime>::get(&bound.ca)
						!= Some(signer.clone())
						|| <Members as MembershipProver>::ring_revision(
							&*indiv_pallet_people::PEOPLE_MEMBER_IDENTIFIER,
							bound.ring,
						) != Some(bound.revision)
					{
						return Err(InvalidTransaction::BadSigner.into());
					}
					(
						indiv_pallet_people::Origin::PersonalAlias(bound),
						PolicyValV6::PersonalAlias(signer.clone()),
					)
				},
				MetaPersonhoodAuthV6::PersonalIdentityAccount => {
					let id = indiv_pallet_people::AccountToPersonalId::<Runtime>::get(&signer)
						.ok_or(InvalidTransaction::BadSigner)?;
					if !indiv_pallet_people::People::<Runtime>::get(id)
						.is_some_and(|record| record.account == Some(signer.clone()))
					{
						return Err(InvalidTransaction::BadSigner.into());
					}
					(
						indiv_pallet_people::Origin::PersonalIdentity(id),
						PolicyValV6::PersonalIdentity(signer.clone()),
					)
				},
				MetaPersonhoodAuthV6::PersonalAliasAccountRevised(proof, ring_index, context) => {
					let old = indiv_pallet_people::AccountToAlias::<Runtime>::get(&signer)
						.ok_or(InvalidTransaction::BadSigner)?;
					let msg = (
						b"orbis/meta/v6/personhood/alias-revised",
						&signer,
						&signer,
						call,
						inherited,
					)
						.using_encoded(sp_io::hashing::blake2_256);
					let revised = <Members as MembershipProver>::verify_membership(
						&*indiv_pallet_people::PEOPLE_MEMBER_IDENTIFIER,
						proof,
						*ring_index,
						*context,
						&msg,
					)
					.map_err(|_| InvalidTransaction::BadProof)?;
					if revised.ca.alias != old.ca.alias
						|| revised.ca.context != old.ca.context
						|| revised.revision <= old.revision
						|| revised.ring != old.ring
						|| revised.ring != *ring_index
						|| <Members as MembershipProver>::ring_revision(
							&*indiv_pallet_people::PEOPLE_MEMBER_IDENTIFIER,
							revised.ring,
						) != Some(revised.revision)
						|| indiv_pallet_people::AliasToAccount::<Runtime>::get(&old.ca)
							!= Some(signer.clone())
					{
						return Err(InvalidTransaction::BadSigner.into());
					}
					(
						indiv_pallet_people::Origin::PersonalAlias(revised.clone()),
						PolicyValV6::PersonRevision(signer.clone(), revised),
					)
				},
			};
			origin.set_caller_from(local);
			return Ok((ValidTransaction::default(), value, origin));
		}
		if let Some(lite) = &self.0.people_lite {
			debug_assert!(matches!(
				route,
				RouterRouteV6::LitePerson
					| RouterRouteV6::LiteAlias
					| RouterRouteV6::LiteAliasRevised
			));
			let (local, value) = match lite {
				MetaPeopleLiteAuthV6::LitePerson => {
					if !indiv_pallet_people_lite::LitePeople::<Runtime>::contains_key(&signer) {
						return Err(InvalidTransaction::BadSigner.into());
					}
					(
						indiv_pallet_people_lite::Origin::LitePerson(signer.clone()),
						PolicyValV6::LitePerson(signer.clone()),
					)
				},
				MetaPeopleLiteAuthV6::LiteAliasAccount => {
					let bound = indiv_pallet_people_lite::AccountToAlias::<Runtime>::get(&signer)
						.ok_or(InvalidTransaction::BadSigner)?;
					if indiv_pallet_people_lite::AliasToAccount::<Runtime>::get(&bound.ca)
						!= Some(signer.clone())
						|| <Members as MembershipProver>::ring_revision(
							&*indiv_pallet_people_lite::LITE_PEOPLE_MEMBER_IDENTIFIER,
							bound.ring,
						) != Some(bound.revision)
					{
						return Err(InvalidTransaction::BadSigner.into());
					}
					(
						indiv_pallet_people_lite::Origin::LiteAlias(bound),
						PolicyValV6::LiteAlias(signer.clone()),
					)
				},
				MetaPeopleLiteAuthV6::LiteAliasAccountRevised(proof, ring_index, context) => {
					let old = indiv_pallet_people_lite::AccountToAlias::<Runtime>::get(&signer)
						.ok_or(InvalidTransaction::BadSigner)?;
					if *context != *indiv_pallet_people_lite::LITE_PEOPLE_AUTH_CONTEXT {
						return Err(InvalidTransaction::Call.into());
					}
					let msg = (
						b"orbis/meta/v6/people-lite/alias-revised",
						&signer,
						&signer,
						call,
						inherited,
					)
						.using_encoded(sp_io::hashing::blake2_256);
					let revised = <Members as MembershipProver>::verify_membership(
						&*indiv_pallet_people_lite::LITE_PEOPLE_MEMBER_IDENTIFIER,
						proof,
						*ring_index,
						*context,
						&msg,
					)
					.map_err(|_| InvalidTransaction::BadProof)?;
					if revised.ca.alias != old.ca.alias
						|| revised.ca.context != old.ca.context
						|| revised.revision <= old.revision
						|| revised.ring != old.ring
						|| revised.ring != *ring_index
						|| <Members as MembershipProver>::ring_revision(
							&*indiv_pallet_people_lite::LITE_PEOPLE_MEMBER_IDENTIFIER,
							revised.ring,
						) != Some(revised.revision)
						|| indiv_pallet_people_lite::AliasToAccount::<Runtime>::get(&old.ca)
							!= Some(signer.clone())
					{
						return Err(InvalidTransaction::BadSigner.into());
					}
					(
						indiv_pallet_people_lite::Origin::LiteAlias(revised.clone()),
						PolicyValV6::LiteRevision(signer.clone(), revised),
					)
				},
			};
			origin.set_caller_from(local);
			return Ok((ValidTransaction::default(), value, origin));
		}
		match call {
			RuntimeCall::Resources(indiv_pallet_resources::Call::claim_long_term_storage {
				period,
				counter,
				account_id,
			}) => {
				if account_id != &signer
					|| self.0.personhood.is_some()
					|| self.0.people_lite.is_some()
				{
					return Err(InvalidTransaction::BadSigner.into());
				}
				let Some(MetaResourcesAuthV6::ClaimLongTermStorage(
					proof,
					ring_index,
					revision,
					collection,
				)) = &self.0.resources
				else {
					return Err(InvalidTransaction::Custom(META_POLICY_INVALIDITY).into());
				};
				if !crate::Resources::is_accepted_long_term_storage_period(*period)
					|| *counter >= crate::ResourcesLongTermStorageClaimsPerPeriod::get()
				{
					return Err(InvalidTransaction::Custom(META_POLICY_INVALIDITY).into());
				}
				let context = crate::Resources::long_term_storage_context(*period, *counter);
				let bound = match collection {
					indiv_pallet_resources::types::MembershipCollection::People => {
						indiv_pallet_people::AccountToAlias::<Runtime>::get(&signer)
					},
					indiv_pallet_resources::types::MembershipCollection::LitePeople => {
						indiv_pallet_people_lite::AccountToAlias::<Runtime>::get(&signer)
					},
				}
				.ok_or(InvalidTransaction::BadSigner)?;
				if bound.ring != *ring_index || bound.revision != *revision {
					return Err(InvalidTransaction::BadSigner.into());
				}
				let msg = (
					RESOURCES_DOMAIN,
					&signer,
					account_id,
					bound.ca.alias,
					period,
					counter,
					collection,
					ring_index,
					revision,
					context,
					call,
					inherited,
				)
					.using_encoded(sp_io::hashing::blake2_256);
				let identifier = match collection {
					indiv_pallet_resources::types::MembershipCollection::People => {
						*indiv_pallet_people::PEOPLE_MEMBER_IDENTIFIER
					},
					indiv_pallet_resources::types::MembershipCollection::LitePeople => {
						*indiv_pallet_people_lite::LITE_PEOPLE_MEMBER_IDENTIFIER
					},
				};
				let validated = <Members as MembershipProver>::verify_membership_at_rev(
					&identifier,
					proof,
					*ring_index,
					*revision,
					context,
					&msg,
				)
				.map_err(|_| InvalidTransaction::BadProof)?;
				if validated.alias != bound.ca.alias {
					return Err(InvalidTransaction::BadSigner.into());
				}
				let alias = bound.ca.alias;
				let reciprocal = match collection {
					indiv_pallet_resources::types::MembershipCollection::People => {
						indiv_pallet_people::AliasToAccount::<Runtime>::get(&bound.ca)
							== Some(signer.clone())
					},
					indiv_pallet_resources::types::MembershipCollection::LitePeople => {
						indiv_pallet_people_lite::AliasToAccount::<Runtime>::get(&bound.ca)
							== Some(signer.clone())
					},
				};
				if !reciprocal {
					return Err(InvalidTransaction::BadSigner.into());
				}
				if indiv_pallet_resources::SpentLongTermStorageAliases::<Runtime>::contains_key(
					indiv_support::utils::BigEndianU32::from(*period),
					alias,
				) {
					return Err(InvalidTransaction::Stale.into());
				}
				origin.set_caller_from(indiv_pallet_resources::Origin::LongTermStorageClaim {
					alias,
					collection: *collection,
					payer: signer.clone(),
				});
				Ok((
					ValidTransaction::with_tag_prefix("OrbisMetaResources")
						.and_provides((period, alias))
						.into(),
					PolicyValV6::ResourcesClaim(signer, alias),
					origin,
				))
			},
			_ if self.0 == PolicyProofsV6::default() => {
				Ok((ValidTransaction::default(), PolicyValV6::None, origin))
			},
			_ => Err(InvalidTransaction::Call.into()),
		}
	}

	fn prepare(
		self,
		value: PolicyValV6,
		_: &RuntimeOrigin,
		_: &RuntimeCall,
		_: &DispatchInfoOf<RuntimeCall>,
		_: usize,
	) -> Result<PolicyPreV6, TransactionValidityError> {
		Ok(match value {
			PolicyValV6::PersonRevision(account, revised) => {
				indiv_pallet_people::AccountToAlias::<Runtime>::insert(&account, &revised);
				PolicyPreV6::PersonRevision(account, revised.revision)
			},
			PolicyValV6::LiteRevision(account, revised) => {
				indiv_pallet_people_lite::AccountToAlias::<Runtime>::insert(&account, &revised);
				PolicyPreV6::LiteRevision(account, revised.revision)
			},
			PolicyValV6::PersonalAlias(account) => PolicyPreV6::PersonalAlias(account),
			PolicyValV6::PersonalIdentity(account) => PolicyPreV6::PersonalIdentity(account),
			PolicyValV6::LitePerson(account) => PolicyPreV6::LitePerson(account),
			PolicyValV6::LiteAlias(account) => PolicyPreV6::LiteAlias(account),
			PolicyValV6::ResourcesClaim(account, alias) => {
				PolicyPreV6::ResourcesClaim(account, alias)
			},
			PolicyValV6::None => PolicyPreV6::None,
		})
	}
}

#[derive(Encode, Decode, DecodeWithMemTracking, Clone, Eq, PartialEq, Debug, TypeInfo)]
pub struct ConsumePaidMetaIngress(pub IntentPreimageV7);

impl TransactionExtension<RuntimeCall> for ConsumePaidMetaIngress {
	const IDENTIFIER: &'static str = "ConsumePaidMetaIngressV7";
	type Implicit = ();
	type Val = H256;
	type Pre = H256;

	fn weight(&self, _: &RuntimeCall) -> Weight {
		<Runtime as frame_system::Config>::DbWeight::get()
			.reads_writes(1, 1)
			.saturating_add(Weight::from_parts(2_000_000, 0))
			.saturating_add(crate::weights::meta_v6::v7_commitment_delta())
	}

	fn validate(
		&self,
		origin: RuntimeOrigin,
		call: &RuntimeCall,
		_: &DispatchInfoOf<RuntimeCall>,
		_: usize,
		_: (),
		_: &impl Implication,
		_: TransactionSource,
	) -> ValidateResult<H256, RuntimeCall> {
		if origin.as_system_origin_signer() != Some(&self.0.inner_signer)
			|| self.0.domain != META_DOMAIN
		{
			return Err(InvalidTransaction::BadSigner.into());
		}
		if self.0.call_hash != H256::from(sp_io::hashing::blake2_256(&call.encode()))
			|| self.0.spec_version != crate::VERSION.spec_version
			|| self.0.transaction_version != crate::VERSION.transaction_version
			|| self.0.genesis_hash != System::block_hash(0)
		{
			return Err(InvalidTransaction::BadProof.into());
		}
		let commitment = self.0.commitment();
		let current = token().ok_or(InvalidTransaction::BadSigner)?;
		if current.intent_commitment != commitment
			|| current.consumed
			|| current.payer == AccountId::new([0; 32])
			|| current.genesis_hash != System::block_hash(0)
			|| current.spec_version != crate::VERSION.spec_version
			|| current.transaction_version != crate::VERSION.transaction_version
		{
			return Err(InvalidTransaction::BadSigner.into());
		}
		Ok((ValidTransaction::default(), current.key(), origin))
	}

	fn prepare(
		self,
		key: H256,
		_: &RuntimeOrigin,
		_: &RuntimeCall,
		_: &DispatchInfoOf<RuntimeCall>,
		_: usize,
	) -> Result<H256, TransactionValidityError> {
		let current = token().ok_or(InvalidTransaction::BadSigner)?;
		if current.key() != key || current.consumed {
			return Err(InvalidTransaction::BadSigner.into());
		}
		clear_token();
		Ok(key)
	}
}

#[derive(Encode, Decode)]
pub enum VerifySignatureMirror {
	Disabled,
	Signed { signature: crate::Signature, account: AccountId },
}

fn hash_encoded(value: &impl Encode) -> H256 {
	H256::from(value.using_encoded(sp_io::hashing::blake2_256))
}

struct FixedOutput {
	buf: arrayvec::ArrayVec<u8, MAX_META_ENCODED_BYTES>,
	overflow: bool,
}

impl FixedOutput {
	fn encode(value: &impl Encode) -> Result<Self, InvalidTransaction> {
		if value.encoded_size() > MAX_META_ENCODED_BYTES {
			return Err(InvalidTransaction::ExhaustsResources);
		}
		let mut output = Self { buf: arrayvec::ArrayVec::new(), overflow: false };
		value.encode_to(&mut output);
		if output.overflow {
			Err(InvalidTransaction::ExhaustsResources)
		} else {
			Ok(output)
		}
	}
}

impl Output for FixedOutput {
	fn write(&mut self, bytes: &[u8]) {
		if self.overflow || self.buf.try_extend_from_slice(bytes).is_err() {
			self.overflow = true;
		}
	}

	fn push_byte(&mut self, byte: u8) {
		if self.overflow || self.buf.try_push(byte).is_err() {
			self.overflow = true;
		}
	}
}

fn decode_meta_intent<R: MetadataImplicitResolver>(
	call: &RuntimeCall,
) -> Result<H256, TransactionValidityError> {
	let RuntimeCall::MetaTx(pallet_meta_tx::Call::dispatch { meta_tx, .. }) = call else {
		return Err(InvalidTransaction::Call.into());
	};
	decode_meta_payload::<R>(
		&FixedOutput::encode(meta_tx).map_err(TransactionValidityError::Invalid)?.buf,
	)
}

fn decode_meta_payload<R: MetadataImplicitResolver>(
	payload: &[u8],
) -> Result<H256, TransactionValidityError> {
	use codec::DecodeAll;
	let (inner_call, extension_version, extension): (
		RuntimeCall,
		ExtensionVersion,
		crate::MetaTxExtension,
	) = DecodeAll::decode_all(&mut payload.as_ref())
		.map_err(|_| TransactionValidityError::Invalid(InvalidTransaction::BadProof))?;
	let (
		verify,
		consume,
		_marker,
		_nonzero,
		_spec,
		_tx,
		_genesis,
		mortality,
		nonce,
		identity_policies,
		storage,
		metadata,
	) = extension;
	let (_score, policies, _honour) = identity_policies;
	let verify_mirror: VerifySignatureMirror = DecodeAll::decode_all(
		&mut FixedOutput::encode(&verify)
			.map_err(TransactionValidityError::Invalid)?
			.buf
			.as_slice(),
	)
	.map_err(|_| TransactionValidityError::Invalid(InvalidTransaction::BadProof))?;
	let VerifySignatureMirror::Signed { account, .. } = verify_mirror else {
		return Err(InvalidTransaction::BadSigner.into());
	};
	let expected = IntentPreimageV7 {
		domain: META_DOMAIN.to_vec(),
		extension_version,
		genesis_hash: System::block_hash(0),
		spec_version: crate::VERSION.spec_version,
		transaction_version: crate::VERSION.transaction_version,
		inner_signer: account,
		call_hash: hash_encoded(&inner_call),
		mortality: mortality.0,
		nonce: nonce.0,
		policy_proofs_hash: hash_encoded(&policies.0),
		storage_extension_hash: hash_encoded(&storage),
		metadata_extension_hash: hash_encoded(&metadata),
		metadata_implicit: R::resolve(&metadata)?,
	};
	if consume.0 != expected
		|| expected.spec_version != 29
		|| expected.transaction_version != 8
		|| matches!(inner_call, RuntimeCall::MetaTx(..))
	{
		return Err(InvalidTransaction::BadProof.into());
	}
	Ok(expected.commitment())
}

pub const MAX_META_ENVELOPE_DEPTH: u32 = 4;
pub const MAX_META_ENVELOPE_CALLS: u32 = 32;
pub const MAX_META_ENCODED_BYTES: usize = 65_536;
#[allow(dead_code)]
pub const MAX_META_PAYLOAD_BYTES: usize =
	MAX_META_ENCODED_BYTES - crate::weights::meta_v6::METADATA_IMPLICIT_MAX_BYTES as usize;

struct InspectionState {
	visited: u32,
	found: Option<H256>,
}

fn inspect_node<R: MetadataImplicitResolver>(
	call: &RuntimeCall,
	depth: u32,
	cached_encoded_size: Option<usize>,
	state: &mut InspectionState,
) -> Result<(), TransactionValidityError> {
	state.visited = state.visited.saturating_add(1);
	if state.visited > MAX_META_ENVELOPE_CALLS || depth > MAX_META_ENVELOPE_DEPTH {
		return Err(InvalidTransaction::ExhaustsResources.into());
	}
	if matches!(call, RuntimeCall::MetaTx(..)) {
		let encoded_size = cached_encoded_size.unwrap_or_else(|| call.encoded_size());
		if encoded_size > MAX_META_ENCODED_BYTES {
			return Err(InvalidTransaction::ExhaustsResources.into());
		}
		let commitment = decode_meta_intent::<R>(call)?;
		if state.found.replace(commitment).is_some() {
			return Err(InvalidTransaction::Call.into());
		}
		return Ok(());
	}
	if matches!(call, RuntimeCall::Multisig(pallet_multisig::Call::approve_as_multi { .. })) {
		return Ok(());
	}
	let denied_child = match call {
		RuntimeCall::Utility(pallet_utility::Call::dispatch_as { call, .. })
		| RuntimeCall::Utility(pallet_utility::Call::dispatch_as_fallible { call, .. })
		| RuntimeCall::Utility(pallet_utility::Call::as_derivative { call, .. })
		| RuntimeCall::Scheduler(pallet_scheduler::Call::schedule { call, .. })
		| RuntimeCall::Scheduler(pallet_scheduler::Call::schedule_named { call, .. })
		| RuntimeCall::Scheduler(pallet_scheduler::Call::schedule_after { call, .. })
		| RuntimeCall::Scheduler(pallet_scheduler::Call::schedule_named_after { call, .. })
		| RuntimeCall::Sudo(pallet_sudo::Call::sudo { call, .. })
		| RuntimeCall::Sudo(pallet_sudo::Call::sudo_unchecked_weight { call, .. })
		| RuntimeCall::Sudo(pallet_sudo::Call::sudo_as { call, .. })
		| RuntimeCall::Revive(pallet_revive::Call::eth_substrate_call { call, .. })
		| RuntimeCall::Revive(pallet_revive::Call::dispatch_as_fallback_account { call, .. }) => {
			Some(call.as_ref())
		},
		_ => None,
	};
	if let Some(child) = denied_child {
		let before = state.found;
		inspect_node::<R>(child, depth.saturating_add(1), None, state)?;
		return if state.found != before { Err(InvalidTransaction::Call.into()) } else { Ok(()) };
	}
	match call {
		RuntimeCall::Utility(pallet_utility::Call::batch { calls })
		| RuntimeCall::Utility(pallet_utility::Call::batch_all { calls })
		| RuntimeCall::Utility(pallet_utility::Call::force_batch { calls }) => {
			for child in calls {
				inspect_node::<R>(child, depth.saturating_add(1), None, state)?;
			}
		},
		RuntimeCall::Proxy(pallet_proxy::Call::proxy { call, .. })
		| RuntimeCall::Proxy(pallet_proxy::Call::proxy_announced { call, .. })
		| RuntimeCall::Multisig(pallet_multisig::Call::as_multi { call, .. })
		| RuntimeCall::Multisig(pallet_multisig::Call::as_multi_threshold_1 { call, .. }) => {
			inspect_node::<R>(call, depth.saturating_add(1), None, state)?
		},
		_ => {},
	}
	Ok(())
}

pub(crate) fn inspect_paid_meta<R: MetadataImplicitResolver>(
	call: &RuntimeCall,
	depth: u32,
) -> Result<Option<H256>, TransactionValidityError> {
	let root_encoded_size = call.encoded_size();
	if root_encoded_size > MAX_META_ENCODED_BYTES {
		return Err(InvalidTransaction::ExhaustsResources.into());
	}
	let mut state = InspectionState { visited: 0, found: None };
	inspect_node::<R>(call, depth, Some(root_encoded_size), &mut state)?;
	Ok(state.found)
}

pub trait MetadataImplicitResolver {
	fn resolve(
		metadata: &frame_metadata_hash_extension::CheckMetadataHash<Runtime>,
	) -> Result<Option<[u8; 32]>, TransactionValidityError>;
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ProductionMetadataImplicitResolver;

impl MetadataImplicitResolver for ProductionMetadataImplicitResolver {
	fn resolve(
		metadata: &frame_metadata_hash_extension::CheckMetadataHash<Runtime>,
	) -> Result<Option<[u8; 32]>, TransactionValidityError> {
		bulletin_pallets_common::resolve_metadata_implicit::<RuntimeCall, _>(metadata)
	}
}

#[derive(Clone, Eq, PartialEq, Encode, Decode, DecodeWithMemTracking)]
pub struct PaidMetaScope<S, R = ProductionMetadataImplicitResolver>(
	pub S,
	#[codec(skip)] core::marker::PhantomData<R>,
);

impl<S: TypeInfo + 'static, R: 'static> TypeInfo for PaidMetaScope<S, R> {
	type Identity = S;
	fn type_info() -> scale_info::Type {
		S::type_info()
	}
}

impl<S: core::fmt::Debug, R> core::fmt::Debug for PaidMetaScope<S, R> {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.debug_tuple("PaidMetaScope").field(&self.0).finish()
	}
}

impl<S, R> From<S> for PaidMetaScope<S, R> {
	fn from(value: S) -> Self {
		Self(value, core::marker::PhantomData)
	}
}

impl<S> PaidMetaScope<S, ProductionMetadataImplicitResolver> {
	pub fn new(value: S) -> Self {
		Self(value, core::marker::PhantomData)
	}
}

pub struct ScopeVal<V> {
	core: V,
	scope: Option<(AccountId, H256, u32)>,
}

#[cfg(test)]
#[allow(dead_code)]
impl<V> ScopeVal<V> {
	pub(crate) fn core_count(&self) -> usize {
		let _ = &self.core;
		1
	}
	pub(crate) fn scope_count(&self) -> usize {
		usize::from(self.scope.is_some())
	}
}

pub struct ScopePre<P> {
	core: P,
	key: Option<H256>,
}

#[cfg(test)]
#[allow(dead_code)]
impl<P> ScopePre<P> {
	pub(crate) fn core_count(&self) -> usize {
		let _ = &self.core;
		1
	}
	pub(crate) fn key_count(&self) -> usize {
		usize::from(self.key.is_some())
	}
}

impl<S, R> TransactionExtension<RuntimeCall> for PaidMetaScope<S, R>
where
	S: TransactionExtension<RuntimeCall>,
	R: MetadataImplicitResolver + Clone + Eq + Send + Sync + 'static,
{
	const IDENTIFIER: &'static str = "PaidMetaScopeV7";
	type Implicit = S::Implicit;
	type Val = ScopeVal<S::Val>;
	type Pre = ScopePre<S::Pre>;

	fn metadata() -> Vec<sp_runtime::traits::TransactionExtensionMetadata> {
		S::metadata()
	}

	fn implicit(&self) -> Result<Self::Implicit, TransactionValidityError> {
		self.0.implicit()
	}

	fn weight(&self, call: &RuntimeCall) -> Weight {
		let measured = <<Runtime as indiv_pallet_resources::Config>::WeightInfo as
			indiv_pallet_resources::weights::WeightInfo>::meta_policy_metadata_max();
		let metadata = measured.max(crate::weights::meta_v6::metadata_outer_implicit());
		self.0
			.weight(call)
			.saturating_add(crate::weights::meta_v6::paid_scope_max(
				<Runtime as frame_system::Config>::DbWeight::get(),
			))
			.saturating_add(metadata)
	}

	fn validate(
		&self,
		origin: RuntimeOrigin,
		call: &RuntimeCall,
		info: &DispatchInfoOf<RuntimeCall>,
		len: usize,
		implicit: Self::Implicit,
		inherited: &impl Implication,
		source: TransactionSource,
	) -> ValidateResult<Self::Val, RuntimeCall> {
		let (valid, core, origin) =
			self.0.validate(origin, call, info, len, implicit, inherited, source)?;
		let intent = inspect_paid_meta::<R>(call, 0)?;
		if intent.is_some()
			&& matches!(origin.as_system_ref(), Some(frame_system::RawOrigin::Authorized))
		{
			return Err(InvalidTransaction::Call.into());
		}
		let scope = if let Some(commitment) = intent {
			let payer =
				origin.as_system_origin_signer().cloned().ok_or(InvalidTransaction::BadSigner)?;
			Some((payer.clone(), commitment, frame_system::Account::<Runtime>::get(&payer).nonce))
		} else {
			None
		};
		Ok((valid, ScopeVal { core, scope }, origin))
	}

	fn prepare(
		self,
		val: Self::Val,
		origin: &RuntimeOrigin,
		call: &RuntimeCall,
		info: &DispatchInfoOf<RuntimeCall>,
		len: usize,
	) -> Result<Self::Pre, TransactionValidityError> {
		if val.scope.is_some() && token().is_some() {
			return Err(InvalidTransaction::ExhaustsResources.into());
		}
		let core = self.0.prepare(val.core, origin, call, info, len)?;
		let key = val.scope.map(|(payer, intent_commitment, outer_nonce)| {
			let token = PaidMetaTokenV7 {
				payer,
				intent_commitment,
				outer_nonce,
				genesis_hash: System::block_hash(0),
				spec_version: crate::VERSION.spec_version,
				transaction_version: crate::VERSION.transaction_version,
				consumed: false,
			};
			let key = token.key();
			put_token(&token);
			key
		});
		Ok(ScopePre { core, key })
	}

	fn post_dispatch_details(
		pre: Self::Pre,
		info: &DispatchInfoOf<RuntimeCall>,
		post_info: &sp_runtime::traits::PostDispatchInfoOf<RuntimeCall>,
		len: usize,
		result: &frame_support::dispatch::DispatchResult,
	) -> Result<Weight, TransactionValidityError> {
		if let (Some(expected), Some(current)) = (pre.key, token()) {
			if current.key() == expected {
				clear_token();
			}
		}
		S::post_dispatch_details(pre.core, info, post_info, len, result)
	}
}

pub struct BaseFilter;

impl frame_support::traits::Contains<RuntimeCall> for BaseFilter {
	fn contains(call: &RuntimeCall) -> bool {
		if !matches!(call, RuntimeCall::MetaTx(..)) {
			return true;
		}
		token().is_some_and(|token| {
			!token.consumed
				&& token.spec_version == crate::VERSION.spec_version
				&& token.transaction_version == crate::VERSION.transaction_version
		})
	}
}

pub struct MetaTokenMustBeEmpty;

impl frame_support::traits::PostTransactions for MetaTokenMustBeEmpty {
	fn post_transactions() {
		assert!(token().is_none(), "Orbis paid Meta token leaked past transaction execution");
	}
}
