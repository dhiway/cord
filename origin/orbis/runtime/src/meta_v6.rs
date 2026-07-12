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

pub const META_DOMAIN: &[u8] = b"orbis/meta-intent/v6";
pub const PAID_META_DOMAIN: &[u8] = b"orbis/paid-meta/v6";
pub const RESOURCES_DOMAIN: &[u8] = b"orbis/meta/v6/resources/long-term-storage";
pub const TOKEN_SLOT: &[u8] = b":orbis:paid-meta:v6";
pub const META_POLICY_INVALIDITY: u8 = 239;

#[derive(Encode, Decode, DecodeWithMemTracking, Clone, Eq, PartialEq, Debug, TypeInfo)]
pub struct IntentPreimageV6 {
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
}

impl IntentPreimageV6 {
	pub fn commitment(&self) -> H256 {
		H256::from(sp_io::hashing::blake2_256(&self.encode()))
	}
}

#[derive(Encode, Decode, DecodeWithMemTracking, Clone, Eq, PartialEq, Debug, TypeInfo)]
pub struct PaidMetaTokenV6 {
	pub payer: AccountId,
	pub intent_commitment: H256,
	pub outer_nonce: u32,
	pub genesis_hash: crate::Hash,
	pub spec_version: u32,
	pub transaction_version: u32,
	pub consumed: bool,
}

impl PaidMetaTokenV6 {
	pub fn key(&self) -> H256 {
		H256::from(sp_io::hashing::blake2_256(&(PAID_META_DOMAIN, self).encode()))
	}
}

pub fn token() -> Option<PaidMetaTokenV6> {
	frame_support::storage::unhashed::get(TOKEN_SLOT)
}

pub fn put_token(token: &PaidMetaTokenV6) {
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
		if self.0.personhood.is_some() as u8 +
			self.0.people_lite.is_some() as u8 +
			self.0.resources.is_some() as u8 >
			1
		{
			return Err(InvalidTransaction::Call);
		}
		let resources_call = matches!(
			call,
			RuntimeCall::Resources(indiv_pallet_resources::Call::claim_long_term_storage { .. })
		);
		match (&self.0.personhood, &self.0.people_lite, &self.0.resources) {
			(None, None, Some(MetaResourcesAuthV6::ClaimLongTermStorage(..))) if resources_call =>
				Ok(RouterRouteV6::ResourcesClaim),
			(_, _, _) if resources_call => Err(InvalidTransaction::Call),
			(Some(MetaPersonhoodAuthV6::PersonalAliasAccount), None, None) =>
				Ok(RouterRouteV6::PersonalAlias),
			(Some(MetaPersonhoodAuthV6::PersonalIdentityAccount), None, None) =>
				Ok(RouterRouteV6::PersonalIdentity),
			(Some(MetaPersonhoodAuthV6::PersonalAliasAccountRevised(..)), None, None) =>
				Ok(RouterRouteV6::PersonalAliasRevised),
			(None, Some(MetaPeopleLiteAuthV6::LitePerson), None) => Ok(RouterRouteV6::LitePerson),
			(None, Some(MetaPeopleLiteAuthV6::LiteAliasAccount), None) =>
				Ok(RouterRouteV6::LiteAlias),
			(None, Some(MetaPeopleLiteAuthV6::LiteAliasAccountRevised(..)), None) =>
				Ok(RouterRouteV6::LiteAliasRevised),
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
) {
	use frame_support::dispatch::GetDispatchInfo;
	use indiv_pallet_resources::benchmarking::MetaPolicyBenchmarkScenario as Scenario;
	use sp_runtime::traits::{TrailingZeroInput, TransactionExtension};

	fn proof<P: Decode>() -> P {
		P::decode(&mut TrailingZeroInput::zeroes())
			.expect("benchmark-only trailing-zero proof has the production SCALE shape")
	}
	fn max_proof<P: Decode>() -> P {
		// Bandersnatch: 752-byte ring proof + one context-count byte + 16 * 32-byte
		// outputs. SCALE adds the compact length prefix to these 1,265 payload bytes.
		P::decode(&mut alloc::vec![0u8; 1_265].encode().as_slice())
			.expect("the production maximum membership proof has a bounded SCALE shape")
	}

	let ordinary = RuntimeCall::System(frame_system::Call::remark { remark: Vec::new() });
	let signer = AccountId::decode(&mut TrailingZeroInput::zeroes())
		.expect("AccountId has a fixed benchmark representation");
	let resources = RuntimeCall::Resources(indiv_pallet_resources::Call::claim_long_term_storage {
		period: 0,
		counter: 0,
		account_id: signer,
	});
	let policies = match scenario {
		Scenario::PersonalAlias | Scenario::MappingMiss => PolicyProofsV6 {
			personhood: Some(MetaPersonhoodAuthV6::PersonalAliasAccount),
			..Default::default()
		},
		Scenario::PersonalIdentity => PolicyProofsV6 {
			personhood: Some(MetaPersonhoodAuthV6::PersonalIdentityAccount),
			..Default::default()
		},
		Scenario::PersonalAliasRevised | Scenario::RevisedWrite => PolicyProofsV6 {
			personhood: Some(MetaPersonhoodAuthV6::PersonalAliasAccountRevised(
				proof(),
				0,
				crate::ORBIS_PERSON_CONTEXT,
			)),
			..Default::default()
		},
		Scenario::LitePerson => PolicyProofsV6 {
			people_lite: Some(MetaPeopleLiteAuthV6::LitePerson),
			..Default::default()
		},
		Scenario::LiteAlias => PolicyProofsV6 {
			people_lite: Some(MetaPeopleLiteAuthV6::LiteAliasAccount),
			..Default::default()
		},
		Scenario::LiteAliasRevised => PolicyProofsV6 {
			people_lite: Some(MetaPeopleLiteAuthV6::LiteAliasAccountRevised(
				proof(),
				0,
				*indiv_pallet_people_lite::LITE_PEOPLE_AUTH_CONTEXT,
			)),
			..Default::default()
		},
		Scenario::ResourcesClaim => PolicyProofsV6 {
			resources: Some(MetaResourcesAuthV6::ClaimLongTermStorage(
				proof(),
				0,
				0,
				indiv_pallet_resources::types::MembershipCollection::People,
			)),
			..Default::default()
		},
		Scenario::MaxProof => PolicyProofsV6 {
			resources: Some(MetaResourcesAuthV6::ClaimLongTermStorage(
				max_proof(),
				0,
				0,
				indiv_pallet_resources::types::MembershipCollection::People,
			)),
			..Default::default()
		},
		Scenario::Malformed => PolicyProofsV6 {
			personhood: Some(MetaPersonhoodAuthV6::PersonalAliasAccount),
			people_lite: Some(MetaPeopleLiteAuthV6::LitePerson),
			resources: None,
		},
		Scenario::Envelope => {
			let weight = crate::weights::meta_v6::paid_scope_max(
				<Runtime as frame_system::Config>::DbWeight::get(),
			);
			core::hint::black_box(weight);
			return
		},
	};
	let call = if matches!(scenario, Scenario::ResourcesClaim | Scenario::MaxProof) {
		&resources
	} else {
		&ordinary
	};
	let policy = MetaAccountBoundPoliciesV6::new(policies);
	// `weight` invokes the production classifier. Encode the complete production policy as part of
	// the max-proof case so FRAME observes its real SCALE input size rather than a guessed
	// constant.
	if matches!(scenario, Scenario::MaxProof) {
		core::hint::black_box(policy.encode());
	}
	core::hint::black_box(policy.weight(call));
	core::hint::black_box(call.get_dispatch_info());
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
		let signer =
			origin.as_system_origin_signer().cloned().ok_or(InvalidTransaction::BadSigner)?;
		if let Some(personhood) = &self.0.personhood {
			debug_assert!(matches!(
				route,
				RouterRouteV6::PersonalAlias |
					RouterRouteV6::PersonalIdentity |
					RouterRouteV6::PersonalAliasRevised
			));
			let (local, value) = match personhood {
				MetaPersonhoodAuthV6::PersonalAliasAccount => {
					let bound = indiv_pallet_people::AccountToAlias::<Runtime>::get(&signer)
						.ok_or(InvalidTransaction::BadSigner)?;
					if !<Runtime as indiv_pallet_people::Config>::AccountContexts::contains(
						&bound.ca.context,
					) || indiv_pallet_people::AliasToAccount::<Runtime>::get(&bound.ca) !=
						Some(signer.clone()) ||
						<Members as MembershipProver>::ring_revision(
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
					if revised.ca.alias != old.ca.alias ||
						revised.ca.context != old.ca.context ||
						revised.revision <= old.revision ||
						revised.ring != old.ring ||
						revised.ring != *ring_index ||
						<Members as MembershipProver>::ring_revision(
							&*indiv_pallet_people::PEOPLE_MEMBER_IDENTIFIER,
							revised.ring,
						) != Some(revised.revision) ||
						indiv_pallet_people::AliasToAccount::<Runtime>::get(&old.ca) !=
							Some(signer.clone())
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
				RouterRouteV6::LitePerson |
					RouterRouteV6::LiteAlias |
					RouterRouteV6::LiteAliasRevised
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
					if indiv_pallet_people_lite::AliasToAccount::<Runtime>::get(&bound.ca) !=
						Some(signer.clone()) ||
						<Members as MembershipProver>::ring_revision(
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
					if revised.ca.alias != old.ca.alias ||
						revised.ca.context != old.ca.context ||
						revised.revision <= old.revision ||
						revised.ring != old.ring ||
						revised.ring != *ring_index ||
						<Members as MembershipProver>::ring_revision(
							&*indiv_pallet_people_lite::LITE_PEOPLE_MEMBER_IDENTIFIER,
							revised.ring,
						) != Some(revised.revision) ||
						indiv_pallet_people_lite::AliasToAccount::<Runtime>::get(&old.ca) !=
							Some(signer.clone())
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
				if account_id != &signer ||
					self.0.personhood.is_some() ||
					self.0.people_lite.is_some()
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
				if !crate::Resources::is_accepted_long_term_storage_period(*period) ||
					*counter >= crate::ResourcesLongTermStorageClaimsPerPeriod::get()
				{
					return Err(InvalidTransaction::Custom(META_POLICY_INVALIDITY).into());
				}
				let context = crate::Resources::long_term_storage_context(*period, *counter);
				let bound = match collection {
					indiv_pallet_resources::types::MembershipCollection::People =>
						indiv_pallet_people::AccountToAlias::<Runtime>::get(&signer),
					indiv_pallet_resources::types::MembershipCollection::LitePeople =>
						indiv_pallet_people_lite::AccountToAlias::<Runtime>::get(&signer),
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
					indiv_pallet_resources::types::MembershipCollection::People =>
						*indiv_pallet_people::PEOPLE_MEMBER_IDENTIFIER,
					indiv_pallet_resources::types::MembershipCollection::LitePeople =>
						*indiv_pallet_people_lite::LITE_PEOPLE_MEMBER_IDENTIFIER,
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
					indiv_pallet_resources::types::MembershipCollection::People =>
						indiv_pallet_people::AliasToAccount::<Runtime>::get(&bound.ca) ==
							Some(signer.clone()),
					indiv_pallet_resources::types::MembershipCollection::LitePeople =>
						indiv_pallet_people_lite::AliasToAccount::<Runtime>::get(&bound.ca) ==
							Some(signer.clone()),
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
			_ if self.0 == PolicyProofsV6::default() =>
				Ok((ValidTransaction::default(), PolicyValV6::None, origin)),
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
			PolicyValV6::ResourcesClaim(account, alias) =>
				PolicyPreV6::ResourcesClaim(account, alias),
			PolicyValV6::None => PolicyPreV6::None,
		})
	}
}

#[derive(Encode, Decode, DecodeWithMemTracking, Clone, Eq, PartialEq, Debug, TypeInfo)]
pub struct ConsumePaidMetaIngress(pub IntentPreimageV6);

impl TransactionExtension<RuntimeCall> for ConsumePaidMetaIngress {
	const IDENTIFIER: &'static str = "ConsumePaidMetaIngressV6";
	type Implicit = ();
	type Val = H256;
	type Pre = H256;

	fn weight(&self, _: &RuntimeCall) -> Weight {
		<Runtime as frame_system::Config>::DbWeight::get()
			.reads_writes(1, 1)
			.saturating_add(Weight::from_parts(2_000_000, 0))
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
		if origin.as_system_origin_signer() != Some(&self.0.inner_signer) ||
			self.0.domain != META_DOMAIN
		{
			return Err(InvalidTransaction::BadSigner.into());
		}
		if self.0.call_hash != H256::from(sp_io::hashing::blake2_256(&call.encode())) ||
			self.0.spec_version != crate::VERSION.spec_version ||
			self.0.transaction_version != crate::VERSION.transaction_version ||
			self.0.genesis_hash != System::block_hash(0)
		{
			return Err(InvalidTransaction::BadProof.into());
		}
		let commitment = self.0.commitment();
		let current = token().ok_or(InvalidTransaction::BadSigner)?;
		if current.intent_commitment != commitment ||
			current.consumed ||
			current.payer == AccountId::new([0; 32]) ||
			current.genesis_hash != System::block_hash(0) ||
			current.spec_version != crate::VERSION.spec_version ||
			current.transaction_version != crate::VERSION.transaction_version
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

fn decode_meta_intent(call: &RuntimeCall) -> Result<H256, InvalidTransaction> {
	use codec::DecodeAll;
	let RuntimeCall::MetaTx(pallet_meta_tx::Call::dispatch { meta_tx, .. }) = call else {
		return Err(InvalidTransaction::Call);
	};
	let (inner_call, extension_version, extension): (
		RuntimeCall,
		ExtensionVersion,
		crate::MetaTxExtension,
	) = DecodeAll::decode_all(&mut FixedOutput::encode(meta_tx)?.buf.as_slice())
		.map_err(|_| InvalidTransaction::BadProof)?;
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
		policies,
		storage,
		metadata,
	) = extension;
	let verify_mirror: VerifySignatureMirror =
		DecodeAll::decode_all(&mut FixedOutput::encode(&verify)?.buf.as_slice())
			.map_err(|_| InvalidTransaction::BadProof)?;
	let VerifySignatureMirror::Signed { account, .. } = verify_mirror else {
		return Err(InvalidTransaction::BadSigner);
	};
	let expected = IntentPreimageV6 {
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
	};
	if consume.0 != expected ||
		expected.spec_version != 27 ||
		expected.transaction_version != 6 ||
		matches!(inner_call, RuntimeCall::MetaTx(..))
	{
		return Err(InvalidTransaction::BadProof);
	}
	Ok(expected.commitment())
}

pub const MAX_META_ENVELOPE_DEPTH: u32 = 4;
pub const MAX_META_ENVELOPE_CALLS: u32 = 32;
pub const MAX_META_ENCODED_BYTES: usize = 65_536;

struct InspectionState {
	visited: u32,
	found: Option<H256>,
}

fn inspect_node(
	call: &RuntimeCall,
	depth: u32,
	cached_encoded_size: Option<usize>,
	state: &mut InspectionState,
) -> Result<(), InvalidTransaction> {
	state.visited = state.visited.saturating_add(1);
	if state.visited > MAX_META_ENVELOPE_CALLS || depth > MAX_META_ENVELOPE_DEPTH {
		return Err(InvalidTransaction::ExhaustsResources);
	}
	if matches!(call, RuntimeCall::MetaTx(..)) {
		let encoded_size = cached_encoded_size.unwrap_or_else(|| call.encoded_size());
		if encoded_size > MAX_META_ENCODED_BYTES {
			return Err(InvalidTransaction::ExhaustsResources);
		}
		let commitment = decode_meta_intent(call)?;
		if state.found.replace(commitment).is_some() {
			return Err(InvalidTransaction::Call);
		}
		return Ok(());
	}
	if matches!(call, RuntimeCall::Multisig(pallet_multisig::Call::approve_as_multi { .. })) {
		return Ok(());
	}
	let denied_child = match call {
		RuntimeCall::Utility(pallet_utility::Call::dispatch_as { call, .. }) |
		RuntimeCall::Utility(pallet_utility::Call::dispatch_as_fallible { call, .. }) |
		RuntimeCall::Utility(pallet_utility::Call::as_derivative { call, .. }) |
		RuntimeCall::Scheduler(pallet_scheduler::Call::schedule { call, .. }) |
		RuntimeCall::Scheduler(pallet_scheduler::Call::schedule_named { call, .. }) |
		RuntimeCall::Scheduler(pallet_scheduler::Call::schedule_after { call, .. }) |
		RuntimeCall::Scheduler(pallet_scheduler::Call::schedule_named_after { call, .. }) |
		RuntimeCall::Sudo(pallet_sudo::Call::sudo { call, .. }) |
		RuntimeCall::Sudo(pallet_sudo::Call::sudo_unchecked_weight { call, .. }) |
		RuntimeCall::Sudo(pallet_sudo::Call::sudo_as { call, .. }) |
		RuntimeCall::Revive(pallet_revive::Call::eth_substrate_call { call, .. }) |
		RuntimeCall::Revive(pallet_revive::Call::dispatch_as_fallback_account { call, .. }) =>
			Some(call.as_ref()),
		_ => None,
	};
	if let Some(child) = denied_child {
		let before = state.found;
		inspect_node(child, depth.saturating_add(1), None, state)?;
		return if state.found != before { Err(InvalidTransaction::Call) } else { Ok(()) };
	}
	match call {
		RuntimeCall::Utility(pallet_utility::Call::batch { calls }) |
		RuntimeCall::Utility(pallet_utility::Call::batch_all { calls }) |
		RuntimeCall::Utility(pallet_utility::Call::force_batch { calls }) =>
			for child in calls {
				inspect_node(child, depth.saturating_add(1), None, state)?;
			},
		RuntimeCall::Proxy(pallet_proxy::Call::proxy { call, .. }) |
		RuntimeCall::Proxy(pallet_proxy::Call::proxy_announced { call, .. }) |
		RuntimeCall::Multisig(pallet_multisig::Call::as_multi { call, .. }) |
		RuntimeCall::Multisig(pallet_multisig::Call::as_multi_threshold_1 { call, .. }) =>
			inspect_node(call, depth.saturating_add(1), None, state)?,
		_ => {},
	}
	Ok(())
}

pub(crate) fn inspect_paid_meta(
	call: &RuntimeCall,
	depth: u32,
) -> Result<Option<H256>, InvalidTransaction> {
	let root_encoded_size = call.encoded_size();
	if root_encoded_size > MAX_META_ENCODED_BYTES {
		return Err(InvalidTransaction::ExhaustsResources);
	}
	let mut state = InspectionState { visited: 0, found: None };
	inspect_node(call, depth, Some(root_encoded_size), &mut state)?;
	Ok(state.found)
}

#[derive(Clone, Eq, PartialEq, Encode, Decode, DecodeWithMemTracking)]
pub struct PaidMetaScope<S>(pub S);

impl<S: TypeInfo + 'static> TypeInfo for PaidMetaScope<S> {
	type Identity = S;
	fn type_info() -> scale_info::Type {
		S::type_info()
	}
}

impl<S: core::fmt::Debug> core::fmt::Debug for PaidMetaScope<S> {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.debug_tuple("PaidMetaScope").field(&self.0).finish()
	}
}

impl<S> From<S> for PaidMetaScope<S> {
	fn from(value: S) -> Self {
		Self(value)
	}
}

pub struct ScopeVal<V> {
	core: V,
	scope: Option<(AccountId, H256, u32)>,
}

pub struct ScopePre<P> {
	core: P,
	key: Option<H256>,
}

impl<S> TransactionExtension<RuntimeCall> for PaidMetaScope<S>
where
	S: TransactionExtension<RuntimeCall>,
{
	const IDENTIFIER: &'static str = "PaidMetaScopeV6";
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
		self.0.weight(call).saturating_add(crate::weights::meta_v6::paid_scope_max(
			<Runtime as frame_system::Config>::DbWeight::get(),
		))
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
		let intent = inspect_paid_meta(call, 0).map_err(TransactionValidityError::Invalid)?;
		if intent.is_some() &&
			matches!(origin.as_system_ref(), Some(frame_system::RawOrigin::Authorized))
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
			let token = PaidMetaTokenV6 {
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
			!token.consumed &&
				token.spec_version == crate::VERSION.spec_version &&
				token.transaction_version == crate::VERSION.transaction_version
		})
	}
}

pub struct MetaTokenMustBeEmpty;

impl frame_support::traits::PostTransactions for MetaTokenMustBeEmpty {
	fn post_transactions() {
		assert!(token().is_none(), "Orbis paid Meta token leaked past transaction execution");
	}
}
