use crate::{AccountId, Members, Runtime, RuntimeCall, RuntimeOrigin, System};
use alloc::vec::Vec;
use codec::{Decode, DecodeWithMemTracking, Encode};
use frame_support::{traits::OriginTrait, weights::Weight};
use indiv_support::traits::{Context, MembershipProver, RevisionIndex, RingIndex};
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

impl MetaAccountBoundPoliciesV6 {
	pub fn new(proofs: PolicyProofsV6) -> Self {
		Self(proofs)
	}
}

impl TransactionExtension<RuntimeCall> for MetaAccountBoundPoliciesV6 {
	const IDENTIFIER: &'static str = "MetaAccountBoundPoliciesV6";
	type Implicit = ();
	type Val = ();
	type Pre = ();

	fn weight(&self, call: &RuntimeCall) -> Weight {
		use indiv_pallet_resources::weights::WeightInfo as _;
		if matches!(
			call,
			RuntimeCall::Resources(indiv_pallet_resources::Call::claim_long_term_storage { .. })
		) {
			<Runtime as indiv_pallet_resources::Config>::WeightInfo::claim_long_term_storage_tx_ext(
			)
		} else {
			<Runtime as frame_system::Config>::DbWeight::get().reads(1)
		}
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
	) -> ValidateResult<(), RuntimeCall> {
		let signer =
			origin.as_system_origin_signer().cloned().ok_or(InvalidTransaction::BadSigner)?;
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
				let msg = (RESOURCES_DOMAIN, &signer, account_id, call, inherited)
					.using_encoded(sp_io::hashing::blake2_256);
				let identifier = match collection {
					indiv_pallet_resources::types::MembershipCollection::People =>
						*indiv_pallet_people::PEOPLE_MEMBER_IDENTIFIER,
					indiv_pallet_resources::types::MembershipCollection::LitePeople =>
						*indiv_pallet_people_lite::LITE_PEOPLE_MEMBER_IDENTIFIER,
				};
				let alias = <Members as MembershipProver>::verify_membership_at_rev(
					&identifier,
					proof,
					*ring_index,
					*revision,
					context,
					&msg,
				)
				.map_err(|_| InvalidTransaction::BadProof)?
				.alias;
				if indiv_pallet_resources::SpentLongTermStorageAliases::<Runtime>::contains_key(
					indiv_support::utils::BigEndianU32::from(*period),
					alias,
				) {
					return Err(InvalidTransaction::Stale.into());
				}
				origin.set_caller_from(indiv_pallet_resources::Origin::LongTermStorageClaim(
					alias,
					*collection,
				));
				Ok((
					ValidTransaction::with_tag_prefix("OrbisMetaResources")
						.and_provides((period, alias))
						.into(),
					(),
					origin,
				))
			},
			_ if self.0 == PolicyProofsV6::default() =>
				Ok((ValidTransaction::default(), (), origin)),
			_ => Err(InvalidTransaction::Call.into()),
		}
	}

	fn prepare(
		self,
		_: (),
		_: &RuntimeOrigin,
		_: &RuntimeCall,
		_: &DispatchInfoOf<RuntimeCall>,
		_: usize,
	) -> Result<(), TransactionValidityError> {
		Ok(())
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

fn decode_meta_intent(call: &RuntimeCall) -> Result<H256, InvalidTransaction> {
	use codec::DecodeAll;
	let RuntimeCall::MetaTx(pallet_meta_tx::Call::dispatch { meta_tx, .. }) = call else {
		return Err(InvalidTransaction::Call);
	};
	let (inner_call, extension_version, extension): (
		RuntimeCall,
		ExtensionVersion,
		crate::MetaTxExtension,
	) = DecodeAll::decode_all(&mut meta_tx.encode().as_slice())
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
		DecodeAll::decode_all(&mut verify.encode().as_slice())
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
		expected.spec_version != 26 ||
		expected.transaction_version != 6 ||
		matches!(inner_call, RuntimeCall::MetaTx(..))
	{
		return Err(InvalidTransaction::BadProof);
	}
	Ok(expected.commitment())
}

pub(crate) fn inspect_paid_meta(
	call: &RuntimeCall,
	depth: u32,
) -> Result<Option<H256>, InvalidTransaction> {
	if depth >= pallet_bulletin_transaction_storage::MAX_WRAPPER_DEPTH {
		return Err(InvalidTransaction::ExhaustsResources);
	}
	if matches!(call, RuntimeCall::MetaTx(..)) {
		return decode_meta_intent(call).map(Some);
	}
	if matches!(call, RuntimeCall::Multisig(pallet_multisig::Call::approve_as_multi { .. })) {
		return Err(InvalidTransaction::Call);
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
		return match inspect_paid_meta(child, depth + 1)? {
			Some(_) => Err(InvalidTransaction::Call),
			None => Ok(None),
		};
	}
	let children: Option<Vec<&RuntimeCall>> = match call {
		RuntimeCall::Utility(pallet_utility::Call::batch { calls }) |
		RuntimeCall::Utility(pallet_utility::Call::batch_all { calls }) |
		RuntimeCall::Utility(pallet_utility::Call::force_batch { calls }) => Some(calls.iter().collect()),
		RuntimeCall::Proxy(pallet_proxy::Call::proxy { call, .. }) |
		RuntimeCall::Proxy(pallet_proxy::Call::proxy_announced { call, .. }) |
		RuntimeCall::Multisig(pallet_multisig::Call::as_multi { call, .. }) |
		RuntimeCall::Multisig(pallet_multisig::Call::as_multi_threshold_1 { call, .. }) =>
			Some(alloc::vec![call.as_ref()]),
		_ => None,
	};
	let Some(children) = children else { return Ok(None) };
	let mut found = None;
	for child in children {
		if let Some(commitment) = inspect_paid_meta(child, depth + 1)? {
			if found.replace(commitment).is_some() {
				return Err(InvalidTransaction::Call);
			}
		}
	}
	Ok(found)
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
		self.0
			.weight(call)
			.saturating_add(<Runtime as frame_system::Config>::DbWeight::get().reads_writes(2, 2))
			.saturating_add(Weight::from_parts(3_000_000, 0))
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
