// This file is part of CORD – https://cord.network

// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Test-composed support contract for the Orbis spec-27 remediation.
//!
//! Gate 2 deliberately includes this module only from `tests.rs`. Nothing in this file is linked
//! into the production runtime until the single Gate-4 composition and spec-version transition.

use codec::{Decode, DecodeAll, Encode};
use sp_core::H256;

pub const META_INTENT_DOMAIN_V6: &[u8] = b"orbis/meta-intent/v6";
pub const PAID_META_DOMAIN_V6: &[u8] = b"orbis/paid-meta/v6";
pub const CANONICAL_SPEC_VERSION: u32 = 27;
pub const CANONICAL_TRANSACTION_VERSION: u32 = 6;
pub const MAX_META_ENVELOPE_DEPTH: u8 = 4;
pub const MAX_META_ENVELOPE_CALLS: usize = 32;
pub const MAX_META_ENCODED_BYTES: usize = 65_536;
pub const META_EXTENSION_ORDER_V6: [&str; 12] = [
	"VerifySignature",
	"ConsumePaidMetaIngress",
	"MetaTxMarker",
	"CheckNonZeroSender",
	"CheckSpecVersion",
	"CheckTxVersion",
	"CheckGenesis",
	"CheckMortality",
	"CheckNonce",
	"MetaAccountBoundPoliciesV6",
	"ValidateStorageCalls",
	"CheckMetadataHash",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IngressDisposition {
	Allowed,
	Denied,
	Ineligible,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IngressShapeV3 {
	Direct,
	UtilityBatch,
	UtilityBatchAll,
	UtilityForceBatch,
	Proxy,
	ProxyAnnounced,
	MultisigAsMulti,
	MultisigThresholdOne,
	NestedMeta,
	MultipleMeta,
	UtilityDispatchAs,
	UtilityAsDerivative,
	Sudo,
	SchedulerOrPreimage,
	MultisigApprovalOnly,
	XcmOrSovereign,
	AuthorizedOrOffchain,
	ReviveOrEthereum,
	Payerless,
	Opaque,
}

impl IngressShapeV3 {
	pub const ALL: [Self; 20] = [
		Self::Direct,
		Self::UtilityBatch,
		Self::UtilityBatchAll,
		Self::UtilityForceBatch,
		Self::Proxy,
		Self::ProxyAnnounced,
		Self::MultisigAsMulti,
		Self::MultisigThresholdOne,
		Self::NestedMeta,
		Self::MultipleMeta,
		Self::UtilityDispatchAs,
		Self::UtilityAsDerivative,
		Self::Sudo,
		Self::SchedulerOrPreimage,
		Self::MultisigApprovalOnly,
		Self::XcmOrSovereign,
		Self::AuthorizedOrOffchain,
		Self::ReviveOrEthereum,
		Self::Payerless,
		Self::Opaque,
	];

	pub const fn disposition(self) -> IngressDisposition {
		match self {
			Self::Direct |
			Self::UtilityBatch |
			Self::UtilityBatchAll |
			Self::UtilityForceBatch |
			Self::Proxy |
			Self::ProxyAnnounced |
			Self::MultisigAsMulti |
			Self::MultisigThresholdOne => IngressDisposition::Allowed,
			Self::MultisigApprovalOnly => IngressDisposition::Ineligible,
			_ => IngressDisposition::Denied,
		}
	}
}

pub type FixtureAccount = [u8; 32];
pub type FixtureHash = [u8; 32];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RouterVariantV6 {
	PersonalAliasAccount,
	PersonalIdentityAccount,
	PersonalAliasAccountRevised,
	LitePerson,
	LiteAliasAccount,
	LiteAliasAccountRevised,
	ClaimLongTermStorage,
}

impl RouterVariantV6 {
	pub const ALL: [Self; 7] = [
		Self::PersonalAliasAccount,
		Self::PersonalIdentityAccount,
		Self::PersonalAliasAccountRevised,
		Self::LitePerson,
		Self::LiteAliasAccount,
		Self::LiteAliasAccountRevised,
		Self::ClaimLongTermStorage,
	];

	pub const fn persists_revision(self) -> bool {
		matches!(self, Self::PersonalAliasAccountRevised | Self::LiteAliasAccountRevised)
	}
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RouterValV6 {
	pub variant: RouterVariantV6,
	pub verified_signer: FixtureAccount,
	pub authority_account: FixtureAccount,
	pub revision: Option<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RouterPreV6 {
	pub variant: RouterVariantV6,
	pub authority_account: FixtureAccount,
	pub revision_to_write: Option<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RouterError {
	SignerAccountMismatch,
	MissingRevision,
	UnexpectedRevision,
}

pub fn validate_router(
	variant: RouterVariantV6,
	verified_signer: FixtureAccount,
	authority_account: FixtureAccount,
	revision: Option<u32>,
) -> Result<RouterValV6, RouterError> {
	if verified_signer != authority_account {
		return Err(RouterError::SignerAccountMismatch);
	}
	match (variant.persists_revision(), revision) {
		(true, None) => return Err(RouterError::MissingRevision),
		(false, Some(_)) => return Err(RouterError::UnexpectedRevision),
		_ => {},
	}
	Ok(RouterValV6 { variant, verified_signer, authority_account, revision })
}

pub fn prepare_router(value: RouterValV6) -> RouterPreV6 {
	RouterPreV6 {
		variant: value.variant,
		authority_account: value.authority_account,
		revision_to_write: value.revision,
	}
}

#[derive(Clone, Copy, Debug, Decode, Encode, Eq, PartialEq)]
pub enum MembershipCollectionV6 {
	#[codec(index = 0)]
	People,
	#[codec(index = 1)]
	LitePeople,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectResourcesOrigin {
	Signed(FixtureAccount),
	None,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthoritativeAliasBindingV3 {
	pub collection: MembershipCollectionV6,
	pub alias: FixtureHash,
	pub alias_to_account: FixtureAccount,
	pub account_to_alias: FixtureHash,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignedDirectResourcesPayerV3 {
	pub origin: DirectResourcesOrigin,
	pub call_account: FixtureAccount,
	pub claim_collection: MembershipCollectionV6,
	pub derived_alias: FixtureHash,
	pub authoritative_binding: Option<AuthoritativeAliasBindingV3>,
	pub signature_is_valid: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectResourcesPayerValV3 {
	pub nonce_owner: FixtureAccount,
	pub payment_owner: FixtureAccount,
	pub call_account: FixtureAccount,
	pub collection: MembershipCollectionV6,
	pub alias: FixtureHash,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectPayerError {
	BadSignature,
	NoneOrigin,
	SignerAccountMismatch,
	UnmappedAlias,
	AuthoritativeMappingMismatch,
}

impl SignedDirectResourcesPayerV3 {
	pub fn validate(self) -> Result<DirectResourcesPayerValV3, DirectPayerError> {
		if !self.signature_is_valid {
			return Err(DirectPayerError::BadSignature);
		}
		let DirectResourcesOrigin::Signed(signer) = self.origin else {
			return Err(DirectPayerError::NoneOrigin);
		};
		if signer != self.call_account {
			return Err(DirectPayerError::SignerAccountMismatch);
		}
		let binding = self.authoritative_binding.ok_or(DirectPayerError::UnmappedAlias)?;
		if binding.collection != self.claim_collection ||
			binding.alias != self.derived_alias ||
			binding.alias_to_account != signer ||
			binding.account_to_alias != self.derived_alias
		{
			return Err(DirectPayerError::AuthoritativeMappingMismatch);
		}
		Ok(DirectResourcesPayerValV3 {
			nonce_owner: signer,
			payment_owner: signer,
			call_account: self.call_account,
			collection: self.claim_collection,
			alias: self.derived_alias,
		})
	}
}

#[derive(Clone, Copy, Debug, Decode, Encode, Eq, PartialEq)]
pub struct RevisedProofFixtureV6 {
	pub proof: FixtureHash,
	pub ring_index: u32,
	pub context: FixtureHash,
}

#[derive(Clone, Copy, Debug, Decode, Encode, Eq, PartialEq)]
pub enum MetaPersonhoodProofMirrorV6 {
	#[codec(index = 0)]
	PersonalAliasAccount,
	#[codec(index = 1)]
	PersonalIdentityAccount,
	#[codec(index = 2)]
	PersonalAliasAccountRevised(RevisedProofFixtureV6),
}

#[derive(Clone, Copy, Debug, Decode, Encode, Eq, PartialEq)]
pub enum MetaPeopleLiteProofMirrorV6 {
	#[codec(index = 0)]
	LitePerson,
	#[codec(index = 1)]
	LiteAliasAccount,
	#[codec(index = 2)]
	LiteAliasAccountRevised(RevisedProofFixtureV6),
}

#[derive(Clone, Copy, Debug, Decode, Encode, Eq, PartialEq)]
pub enum MetaResourcesProofMirrorV6 {
	#[codec(index = 0)]
	ClaimLongTermStorage {
		proof: FixtureHash,
		ring_index: u32,
		retained_revision: u32,
		collection: MembershipCollectionV6,
	},
}

#[derive(Clone, Copy, Debug, Decode, Encode, Eq, PartialEq, Default)]
pub struct PolicyProofsMirrorV6 {
	pub personhood: Option<MetaPersonhoodProofMirrorV6>,
	pub people_lite: Option<MetaPeopleLiteProofMirrorV6>,
	pub resources: Option<MetaResourcesProofMirrorV6>,
}

impl PolicyProofsMirrorV6 {
	pub fn populated_slots(&self) -> u8 {
		self.personhood.is_some() as u8 +
			self.people_lite.is_some() as u8 +
			self.resources.is_some() as u8
	}

	pub fn single_variant(&self) -> Option<RouterVariantV6> {
		if self.populated_slots() != 1 {
			return None;
		}
		match (self.personhood, self.people_lite, self.resources) {
			(Some(MetaPersonhoodProofMirrorV6::PersonalAliasAccount), None, None) =>
				Some(RouterVariantV6::PersonalAliasAccount),
			(Some(MetaPersonhoodProofMirrorV6::PersonalIdentityAccount), None, None) =>
				Some(RouterVariantV6::PersonalIdentityAccount),
			(Some(MetaPersonhoodProofMirrorV6::PersonalAliasAccountRevised(_)), None, None) =>
				Some(RouterVariantV6::PersonalAliasAccountRevised),
			(None, Some(MetaPeopleLiteProofMirrorV6::LitePerson), None) =>
				Some(RouterVariantV6::LitePerson),
			(None, Some(MetaPeopleLiteProofMirrorV6::LiteAliasAccount), None) =>
				Some(RouterVariantV6::LiteAliasAccount),
			(None, Some(MetaPeopleLiteProofMirrorV6::LiteAliasAccountRevised(_)), None) =>
				Some(RouterVariantV6::LiteAliasAccountRevised),
			(None, None, Some(MetaResourcesProofMirrorV6::ClaimLongTermStorage { .. })) =>
				Some(RouterVariantV6::ClaimLongTermStorage),
			_ => None,
		}
	}
}

pub fn policy_proof_fixture(variant: RouterVariantV6) -> PolicyProofsMirrorV6 {
	let revised = RevisedProofFixtureV6 { proof: [0x71; 32], ring_index: 5, context: [0x72; 32] };
	match variant {
		RouterVariantV6::PersonalAliasAccount => PolicyProofsMirrorV6 {
			personhood: Some(MetaPersonhoodProofMirrorV6::PersonalAliasAccount),
			..Default::default()
		},
		RouterVariantV6::PersonalIdentityAccount => PolicyProofsMirrorV6 {
			personhood: Some(MetaPersonhoodProofMirrorV6::PersonalIdentityAccount),
			..Default::default()
		},
		RouterVariantV6::PersonalAliasAccountRevised => PolicyProofsMirrorV6 {
			personhood: Some(MetaPersonhoodProofMirrorV6::PersonalAliasAccountRevised(revised)),
			..Default::default()
		},
		RouterVariantV6::LitePerson => PolicyProofsMirrorV6 {
			people_lite: Some(MetaPeopleLiteProofMirrorV6::LitePerson),
			..Default::default()
		},
		RouterVariantV6::LiteAliasAccount => PolicyProofsMirrorV6 {
			people_lite: Some(MetaPeopleLiteProofMirrorV6::LiteAliasAccount),
			..Default::default()
		},
		RouterVariantV6::LiteAliasAccountRevised => PolicyProofsMirrorV6 {
			people_lite: Some(MetaPeopleLiteProofMirrorV6::LiteAliasAccountRevised(revised)),
			..Default::default()
		},
		RouterVariantV6::ClaimLongTermStorage => PolicyProofsMirrorV6 {
			resources: Some(MetaResourcesProofMirrorV6::ClaimLongTermStorage {
				proof: [0x73; 32],
				ring_index: 6,
				retained_revision: 9,
				collection: MembershipCollectionV6::People,
			}),
			..Default::default()
		},
	}
}

#[derive(Clone, Debug, Decode, Encode, Eq, PartialEq)]
pub struct IntentPreimageFixtureV6 {
	pub domain: Vec<u8>,
	pub extension_version: u8,
	pub genesis_hash: FixtureHash,
	pub spec_version: u32,
	pub transaction_version: u32,
	pub inner_signer: FixtureAccount,
	pub call_hash: FixtureHash,
	pub mortality: (u64, u64),
	pub nonce: u32,
	pub policy_proofs_hash: FixtureHash,
	pub storage_extension_hash: FixtureHash,
	pub metadata_extension_hash: FixtureHash,
}

impl IntentPreimageFixtureV6 {
	pub fn commitment(&self) -> H256 {
		H256::from(sp_io::hashing::blake2_256(&self.encode()))
	}

	pub fn is_canonical_positive(&self) -> bool {
		self.domain == META_INTENT_DOMAIN_V6 &&
			self.spec_version == CANONICAL_SPEC_VERSION &&
			self.transaction_version == CANONICAL_TRANSACTION_VERSION
	}
}

pub fn canonical_intent_fixture() -> IntentPreimageFixtureV6 {
	let proofs = policy_proof_fixture(RouterVariantV6::ClaimLongTermStorage);
	IntentPreimageFixtureV6 {
		domain: META_INTENT_DOMAIN_V6.to_vec(),
		extension_version: 0,
		genesis_hash: [0x11; 32],
		spec_version: CANONICAL_SPEC_VERSION,
		transaction_version: CANONICAL_TRANSACTION_VERSION,
		inner_signer: [0x22; 32],
		call_hash: [0x33; 32],
		mortality: (64, 7),
		nonce: 9,
		policy_proofs_hash: sp_io::hashing::blake2_256(&proofs.encode()),
		storage_extension_hash: [0x55; 32],
		metadata_extension_hash: [0x66; 32],
	}
}

pub fn canonical_intent_vector() -> (Vec<u8>, H256) {
	let fixture = canonical_intent_fixture();
	(fixture.encode(), fixture.commitment())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnvelopeKind {
	Ordinary,
	MetaLeaf,
	UtilityBatch,
	UtilityBatchAll,
	UtilityForceBatch,
	Proxy,
	ProxyAnnounced,
	MultisigAsMulti,
	MultisigThresholdOne,
	MultisigApprovalOnly,
	DeniedOrOpaque,
}

impl EnvelopeKind {
	const fn may_descend(self) -> bool {
		matches!(
			self,
			Self::UtilityBatch |
				Self::UtilityBatchAll |
				Self::UtilityForceBatch |
				Self::Proxy | Self::ProxyAnnounced |
				Self::MultisigAsMulti |
				Self::MultisigThresholdOne
		)
	}
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EnvelopeNode {
	pub kind: EnvelopeKind,
	pub encoded_bytes: usize,
	pub first_child: u8,
	pub child_count: u8,
}

impl EnvelopeNode {
	pub const fn leaf(kind: EnvelopeKind, encoded_bytes: usize) -> Self {
		Self { kind, encoded_bytes, first_child: 0, child_count: 0 }
	}

	pub const fn wrapper(
		kind: EnvelopeKind,
		encoded_bytes: usize,
		first_child: u8,
		child_count: u8,
	) -> Self {
		Self { kind, encoded_bytes, first_child, child_count }
	}
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InspectionDecision {
	NoMeta,
	EligibleMeta { leaf_index: u8, visited_calls: u8, max_depth: u8 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InspectionError {
	EmptyArena,
	DepthExceeded,
	CallCountExceeded,
	EncodedBytesExceeded,
	MalformedChildren,
	MultipleMetaLeaves,
	NestedMeta,
	DeniedOrOpaque,
}

/// Fixed-stack, allocation-free traversal over a pre-built call arena.
///
/// Root depth is zero. Child ranges are indices into `nodes`; every visited wrapper, sibling and
/// leaf counts against the call bound. Encoded sizes are supplied once by the caller.
pub fn inspect_envelope(nodes: &[EnvelopeNode]) -> Result<InspectionDecision, InspectionError> {
	if nodes.is_empty() {
		return Err(InspectionError::EmptyArena);
	}
	if nodes[0].encoded_bytes > MAX_META_ENCODED_BYTES {
		return Err(InspectionError::EncodedBytesExceeded);
	}
	let mut stack = [(0u8, 0u8); MAX_META_ENVELOPE_CALLS];
	let mut stack_len = 1usize;
	let mut visited = 0usize;
	let mut max_depth = 0u8;
	let mut meta_leaf = None;
	while stack_len != 0 {
		stack_len -= 1;
		let (index, depth) = stack[stack_len];
		let Some(node) = nodes.get(index as usize) else {
			return Err(InspectionError::MalformedChildren);
		};
		visited = visited.saturating_add(1);
		if visited > MAX_META_ENVELOPE_CALLS {
			return Err(InspectionError::CallCountExceeded);
		}
		if depth > MAX_META_ENVELOPE_DEPTH {
			return Err(InspectionError::DepthExceeded);
		}
		max_depth = max_depth.max(depth);
		if node.kind == EnvelopeKind::DeniedOrOpaque {
			return Err(InspectionError::DeniedOrOpaque);
		}
		if node.kind == EnvelopeKind::MetaLeaf {
			if node.encoded_bytes > MAX_META_ENCODED_BYTES {
				return Err(InspectionError::EncodedBytesExceeded);
			}
			if meta_leaf.replace(index).is_some() {
				return Err(InspectionError::MultipleMetaLeaves);
			}
			if node.child_count != 0 {
				return Err(InspectionError::NestedMeta);
			}
			continue;
		}
		if node.child_count == 0 {
			continue;
		}
		if !node.kind.may_descend() {
			return Err(InspectionError::DeniedOrOpaque);
		}
		let start = node.first_child as usize;
		let end = start.saturating_add(node.child_count as usize);
		if end > nodes.len() || end > u8::MAX as usize {
			return Err(InspectionError::MalformedChildren);
		}
		for child in (start..end).rev() {
			if stack_len == MAX_META_ENVELOPE_CALLS {
				return Err(InspectionError::CallCountExceeded);
			}
			stack[stack_len] = (child as u8, depth.saturating_add(1));
			stack_len += 1;
		}
	}
	Ok(match meta_leaf {
		Some(leaf_index) =>
			InspectionDecision::EligibleMeta { leaf_index, visited_calls: visited as u8, max_depth },
		None => InspectionDecision::NoMeta,
	})
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConservativeWeightCoefficients {
	pub base: u64,
	pub per_call: u64,
	pub per_depth: u64,
	pub per_64_bytes: u64,
	pub mirror_decode_hash: u64,
	pub db_read: u64,
	pub db_write: u64,
	pub crypto_verify: u64,
	pub hash_op: u64,
	pub per_proof_byte: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RouterWeightDimensionsV6 {
	pub reads: u64,
	pub writes: u64,
	pub crypto_verifications: u64,
	pub hash_operations: u64,
	pub proof_bytes: usize,
	pub retained_revision_reads: u64,
}

impl RouterWeightDimensionsV6 {
	pub const fn for_variant(variant: RouterVariantV6) -> Self {
		match variant {
			RouterVariantV6::PersonalAliasAccount => Self {
				reads: 2,
				writes: 0,
				crypto_verifications: 0,
				hash_operations: 1,
				proof_bytes: 0,
				retained_revision_reads: 0,
			},
			RouterVariantV6::PersonalIdentityAccount => Self {
				reads: 2,
				writes: 0,
				crypto_verifications: 0,
				hash_operations: 1,
				proof_bytes: 0,
				retained_revision_reads: 0,
			},
			RouterVariantV6::PersonalAliasAccountRevised => Self {
				reads: 3,
				writes: 1,
				crypto_verifications: 1,
				hash_operations: 2,
				proof_bytes: 32,
				retained_revision_reads: 1,
			},
			RouterVariantV6::LitePerson => Self {
				reads: 2,
				writes: 0,
				crypto_verifications: 0,
				hash_operations: 1,
				proof_bytes: 0,
				retained_revision_reads: 0,
			},
			RouterVariantV6::LiteAliasAccount => Self {
				reads: 2,
				writes: 0,
				crypto_verifications: 0,
				hash_operations: 1,
				proof_bytes: 0,
				retained_revision_reads: 0,
			},
			RouterVariantV6::LiteAliasAccountRevised => Self {
				reads: 3,
				writes: 1,
				crypto_verifications: 1,
				hash_operations: 2,
				proof_bytes: 32,
				retained_revision_reads: 1,
			},
			RouterVariantV6::ClaimLongTermStorage => Self {
				reads: 6,
				writes: 0,
				crypto_verifications: 1,
				hash_operations: 3,
				proof_bytes: 32,
				retained_revision_reads: 1,
			},
		}
	}
}

impl ConservativeWeightCoefficients {
	pub const fn fixture() -> Self {
		Self {
			base: 5_000_000,
			per_call: 500_000,
			per_depth: 250_000,
			per_64_bytes: 25_000,
			mirror_decode_hash: 4_000_000,
			db_read: 1_000_000,
			db_write: 2_000_000,
			crypto_verify: 8_000_000,
			hash_op: 500_000,
			per_proof_byte: 10_000,
		}
	}

	pub fn paid_meta_scope(&self, calls: usize, depth: u8, encoded_bytes: usize) -> u64 {
		let chunks = encoded_bytes.saturating_add(63) / 64;
		self.base
			.saturating_add(self.per_call.saturating_mul(calls as u64))
			.saturating_add(self.per_depth.saturating_mul(depth as u64))
			.saturating_add(self.per_64_bytes.saturating_mul(chunks as u64))
			.saturating_add(self.mirror_decode_hash)
			.saturating_add(self.db_read.saturating_mul(2))
			.saturating_add(self.db_write.saturating_mul(2))
	}

	pub fn maximum_rejection(&self) -> u64 {
		self.paid_meta_scope(
			MAX_META_ENVELOPE_CALLS,
			MAX_META_ENVELOPE_DEPTH,
			MAX_META_ENCODED_BYTES,
		)
	}

	pub fn base_leaf(&self) -> u64 {
		self.db_read
	}

	pub fn consume(&self) -> u64 {
		self.db_read.saturating_add(self.db_write)
	}

	pub fn router_variant(&self, variant: RouterVariantV6) -> u64 {
		let dimensions = RouterWeightDimensionsV6::for_variant(variant);
		self.base
			.saturating_add(self.db_read.saturating_mul(dimensions.reads))
			.saturating_add(self.db_write.saturating_mul(dimensions.writes))
			.saturating_add(self.crypto_verify.saturating_mul(dimensions.crypto_verifications))
			.saturating_add(self.hash_op.saturating_mul(dimensions.hash_operations))
			.saturating_add(self.per_proof_byte.saturating_mul(dimensions.proof_bytes as u64))
	}

	pub fn malformed_classifier_and_router(&self) -> u64 {
		let maximum_router = RouterVariantV6::ALL
			.into_iter()
			.map(|variant| self.router_variant(variant))
			.max()
			.unwrap_or_default();
		self.maximum_rejection().saturating_add(maximum_router)
	}
}

#[derive(Clone, Debug, Decode, Encode, Eq, PartialEq)]
pub struct PaidMetaTokenFixtureV6 {
	pub payer: FixtureAccount,
	pub intent_commitment: H256,
	pub outer_nonce: u32,
	pub genesis_hash: FixtureHash,
	pub spec_version: u32,
	pub transaction_version: u32,
	pub consumed: bool,
}

impl PaidMetaTokenFixtureV6 {
	pub fn key(&self) -> H256 {
		H256::from(sp_io::hashing::blake2_256(&(PAID_META_DOMAIN_V6, self).encode()))
	}
}

#[derive(Clone, Copy, Debug, Decode, Encode, Eq, PartialEq)]
pub struct PaidMetaScopeValV6 {
	pub token_key: H256,
	pub payer: FixtureAccount,
	pub intent_commitment: H256,
	pub outer_nonce: u32,
}

#[derive(Clone, Copy, Debug, Decode, Encode, Eq, PartialEq)]
pub struct ConsumePaidMetaPreV6 {
	pub token_key: H256,
	pub payer: FixtureAccount,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenError {
	Occupied,
	Missing,
	KeyMismatch,
	AlreadyConsumed,
	LeftoverAtFinalization,
}

#[derive(Default)]
pub struct PaidMetaTokenSlotV6(Option<PaidMetaTokenFixtureV6>);

impl PaidMetaTokenSlotV6 {
	pub fn prepare_outer(
		&mut self,
		token: PaidMetaTokenFixtureV6,
	) -> Result<PaidMetaScopeValV6, TokenError> {
		if self.0.is_some() {
			return Err(TokenError::Occupied);
		}
		let value = PaidMetaScopeValV6 {
			token_key: token.key(),
			payer: token.payer,
			intent_commitment: token.intent_commitment,
			outer_nonce: token.outer_nonce,
		};
		self.0 = Some(token);
		Ok(value)
	}

	pub fn consume_inner(
		&mut self,
		value: PaidMetaScopeValV6,
	) -> Result<ConsumePaidMetaPreV6, TokenError> {
		let token = self.0.as_ref().ok_or(TokenError::Missing)?;
		if token.consumed {
			return Err(TokenError::AlreadyConsumed);
		}
		if token.key() != value.token_key ||
			token.payer != value.payer ||
			token.intent_commitment != value.intent_commitment ||
			token.outer_nonce != value.outer_nonce
		{
			return Err(TokenError::KeyMismatch);
		}
		self.0.take();
		Ok(ConsumePaidMetaPreV6 { token_key: value.token_key, payer: value.payer })
	}

	pub fn cleanup_matching(&mut self, expected_key: H256) {
		if self.0.as_ref().is_some_and(|token| token.key() == expected_key) {
			self.0.take();
		}
	}

	pub fn post_transactions(&self) -> Result<(), TokenError> {
		if self.0.is_some() {
			Err(TokenError::LeftoverAtFinalization)
		} else {
			Ok(())
		}
	}
}

pub struct MetaTokenMustBeEmptyV3;

impl MetaTokenMustBeEmptyV3 {
	pub fn assert(slot: &PaidMetaTokenSlotV6) -> Result<(), TokenError> {
		slot.post_transactions()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn token() -> PaidMetaTokenFixtureV6 {
		PaidMetaTokenFixtureV6 {
			payer: [7; 32],
			intent_commitment: canonical_intent_fixture().commitment(),
			outer_nonce: 3,
			genesis_hash: [1; 32],
			spec_version: CANONICAL_SPEC_VERSION,
			transaction_version: CANONICAL_TRANSACTION_VERSION,
			consumed: false,
		}
	}

	#[test]
	fn all_seven_router_variants_have_exact_val_and_pre_contracts() {
		assert_eq!(RouterVariantV6::ALL.len(), 7);
		for variant in RouterVariantV6::ALL {
			let proofs = policy_proof_fixture(variant);
			assert_eq!(proofs.populated_slots(), 1);
			assert_eq!(proofs.single_variant(), Some(variant));
			assert_eq!(
				PolicyProofsMirrorV6::decode_all(&mut proofs.encode().as_slice()).unwrap(),
				proofs
			);
			let revision = variant.persists_revision().then_some(11);
			let value = validate_router(variant, [9; 32], [9; 32], revision).unwrap();
			let prepared = prepare_router(value);
			assert_eq!(prepared.variant, variant);
			assert_eq!(prepared.authority_account, [9; 32]);
			assert_eq!(prepared.revision_to_write, revision);
			assert_eq!(
				validate_router(variant, [9; 32], [8; 32], revision),
				Err(RouterError::SignerAccountMismatch)
			);
		}
		let multi_slot = PolicyProofsMirrorV6 {
			personhood: Some(MetaPersonhoodProofMirrorV6::PersonalAliasAccount),
			people_lite: Some(MetaPeopleLiteProofMirrorV6::LitePerson),
			resources: None,
		};
		assert_eq!(multi_slot.populated_slots(), 2);
		assert_eq!(multi_slot.single_variant(), None);
		assert_eq!(PolicyProofsMirrorV6::default().single_variant(), None);
		assert_eq!(
			policy_proof_fixture(RouterVariantV6::PersonalAliasAccount).encode()[..2],
			[1, 0]
		);
		assert_eq!(policy_proof_fixture(RouterVariantV6::LitePerson).encode()[..3], [0, 1, 0]);
		assert_eq!(
			policy_proof_fixture(RouterVariantV6::ClaimLongTermStorage).encode()[..4],
			[0, 0, 1, 0]
		);
	}

	#[test]
	fn signed_direct_resources_payer_contract_is_account_aware() {
		let mapped = SignedDirectResourcesPayerV3 {
			origin: DirectResourcesOrigin::Signed([4; 32]),
			call_account: [4; 32],
			claim_collection: MembershipCollectionV6::People,
			derived_alias: [8; 32],
			authoritative_binding: Some(AuthoritativeAliasBindingV3 {
				collection: MembershipCollectionV6::People,
				alias: [8; 32],
				alias_to_account: [4; 32],
				account_to_alias: [8; 32],
			}),
			signature_is_valid: true,
		};
		let valid = mapped.validate().unwrap();
		assert_eq!(valid.nonce_owner, [4; 32]);
		assert_eq!(valid.payment_owner, [4; 32]);
		assert_eq!(valid.collection, MembershipCollectionV6::People);
		assert_eq!(valid.alias, [8; 32]);
		for (request, error) in [
			(
				SignedDirectResourcesPayerV3 { signature_is_valid: false, ..mapped },
				DirectPayerError::BadSignature,
			),
			(
				SignedDirectResourcesPayerV3 { call_account: [5; 32], ..mapped },
				DirectPayerError::SignerAccountMismatch,
			),
			(
				SignedDirectResourcesPayerV3 { origin: DirectResourcesOrigin::None, ..mapped },
				DirectPayerError::NoneOrigin,
			),
			(
				SignedDirectResourcesPayerV3 { authoritative_binding: None, ..mapped },
				DirectPayerError::UnmappedAlias,
			),
			(
				SignedDirectResourcesPayerV3 {
					authoritative_binding: Some(AuthoritativeAliasBindingV3 {
						account_to_alias: [9; 32],
						..mapped.authoritative_binding.unwrap()
					}),
					..mapped
				},
				DirectPayerError::AuthoritativeMappingMismatch,
			),
			(
				SignedDirectResourcesPayerV3 {
					claim_collection: MembershipCollectionV6::LitePeople,
					..mapped
				},
				DirectPayerError::AuthoritativeMappingMismatch,
			),
		] {
			assert_eq!(request.validate(), Err(error));
		}
	}

	#[test]
	fn canonical_spec27_layout_is_decode_all_and_spec26_is_negative() {
		assert_eq!(
			META_EXTENSION_ORDER_V6,
			[
				"VerifySignature",
				"ConsumePaidMetaIngress",
				"MetaTxMarker",
				"CheckNonZeroSender",
				"CheckSpecVersion",
				"CheckTxVersion",
				"CheckGenesis",
				"CheckMortality",
				"CheckNonce",
				"MetaAccountBoundPoliciesV6",
				"ValidateStorageCalls",
				"CheckMetadataHash",
			]
		);
		let (bytes, commitment) = canonical_intent_vector();
		let decoded = IntentPreimageFixtureV6::decode_all(&mut bytes.as_slice()).unwrap();
		assert_eq!(decoded, canonical_intent_fixture());
		assert_eq!(decoded.commitment(), commitment);
		assert!(decoded.is_canonical_positive());
		let mut with_trailing = bytes.clone();
		with_trailing.push(0);
		assert!(IntentPreimageFixtureV6::decode_all(&mut with_trailing.as_slice()).is_err());
		let mut spec26 = decoded;
		spec26.spec_version = 26;
		assert!(!spec26.is_canonical_positive());
		assert_ne!(spec26.commitment(), commitment);
	}

	#[test]
	fn inspector_is_fixed_stack_bounded_and_enforces_grammar() {
		assert_eq!(IngressShapeV3::ALL.len(), 20);
		assert_eq!(
			IngressShapeV3::ALL
				.into_iter()
				.filter(|shape| shape.disposition() == IngressDisposition::Allowed)
				.count(),
			8
		);
		assert_eq!(
			IngressShapeV3::MultisigApprovalOnly.disposition(),
			IngressDisposition::Ineligible
		);
		assert!(
			IngressShapeV3::ALL
				.into_iter()
				.filter(|shape| shape.disposition() == IngressDisposition::Denied)
				.count() >= 11
		);
		let direct = [EnvelopeNode::leaf(EnvelopeKind::MetaLeaf, 1024)];
		assert_eq!(
			inspect_envelope(&direct),
			Ok(InspectionDecision::EligibleMeta { leaf_index: 0, visited_calls: 1, max_depth: 0 })
		);
		let wrapped = [
			EnvelopeNode::wrapper(EnvelopeKind::UtilityBatchAll, 2048, 1, 2),
			EnvelopeNode::leaf(EnvelopeKind::Ordinary, 10),
			EnvelopeNode::wrapper(EnvelopeKind::ProxyAnnounced, 1024, 3, 1),
			EnvelopeNode::leaf(EnvelopeKind::MetaLeaf, 900),
		];
		assert_eq!(
			inspect_envelope(&wrapped),
			Ok(InspectionDecision::EligibleMeta { leaf_index: 3, visited_calls: 4, max_depth: 2 })
		);
		let multiple = [
			EnvelopeNode::wrapper(EnvelopeKind::UtilityBatch, 200, 1, 2),
			EnvelopeNode::leaf(EnvelopeKind::MetaLeaf, 10),
			EnvelopeNode::leaf(EnvelopeKind::MetaLeaf, 10),
		];
		assert_eq!(inspect_envelope(&multiple), Err(InspectionError::MultipleMetaLeaves));
		assert_eq!(
			inspect_envelope(&[EnvelopeNode::leaf(EnvelopeKind::DeniedOrOpaque, 10,)]),
			Err(InspectionError::DeniedOrOpaque)
		);
		assert_eq!(
			inspect_envelope(&[EnvelopeNode::leaf(
				EnvelopeKind::MetaLeaf,
				MAX_META_ENCODED_BYTES + 1,
			)]),
			Err(InspectionError::EncodedBytesExceeded)
		);
		assert_eq!(
			inspect_envelope(&[EnvelopeNode::leaf(EnvelopeKind::MultisigApprovalOnly, 10)]),
			Ok(InspectionDecision::NoMeta)
		);
		for kind in [
			EnvelopeKind::UtilityBatch,
			EnvelopeKind::UtilityBatchAll,
			EnvelopeKind::UtilityForceBatch,
			EnvelopeKind::Proxy,
			EnvelopeKind::ProxyAnnounced,
			EnvelopeKind::MultisigAsMulti,
			EnvelopeKind::MultisigThresholdOne,
		] {
			assert!(matches!(
				inspect_envelope(&[
					EnvelopeNode::wrapper(kind, 100, 1, 1),
					EnvelopeNode::leaf(EnvelopeKind::MetaLeaf, 50),
				]),
				Ok(InspectionDecision::EligibleMeta { leaf_index: 1, .. })
			));
		}
		let too_deep = [
			EnvelopeNode::wrapper(EnvelopeKind::Proxy, 100, 1, 1),
			EnvelopeNode::wrapper(EnvelopeKind::Proxy, 90, 2, 1),
			EnvelopeNode::wrapper(EnvelopeKind::Proxy, 80, 3, 1),
			EnvelopeNode::wrapper(EnvelopeKind::Proxy, 70, 4, 1),
			EnvelopeNode::wrapper(EnvelopeKind::Proxy, 60, 5, 1),
			EnvelopeNode::leaf(EnvelopeKind::MetaLeaf, 50),
		];
		assert_eq!(inspect_envelope(&too_deep), Err(InspectionError::DepthExceeded));
		let mut too_many = vec![EnvelopeNode::leaf(EnvelopeKind::Ordinary, 1); 33];
		too_many[0] = EnvelopeNode::wrapper(EnvelopeKind::UtilityBatch, 33, 1, 32);
		too_many[32] = EnvelopeNode::leaf(EnvelopeKind::MetaLeaf, 1);
		assert_eq!(inspect_envelope(&too_many), Err(InspectionError::CallCountExceeded));
	}

	#[test]
	fn conservative_weights_freeze_formula_and_db_ownership() {
		let weights = ConservativeWeightCoefficients::fixture();
		assert_eq!(weights.base_leaf(), weights.db_read);
		assert_eq!(weights.consume(), weights.db_read + weights.db_write);
		assert_eq!(weights.maximum_rejection(), weights.paid_meta_scope(32, 4, 65_536));
		assert!(weights.maximum_rejection() > weights.paid_meta_scope(1, 0, 64));
		for variant in RouterVariantV6::ALL {
			let dimensions = RouterWeightDimensionsV6::for_variant(variant);
			let exact = weights
				.base
				.saturating_add(weights.db_read.saturating_mul(dimensions.reads))
				.saturating_add(weights.db_write.saturating_mul(dimensions.writes))
				.saturating_add(
					weights.crypto_verify.saturating_mul(dimensions.crypto_verifications),
				)
				.saturating_add(weights.hash_op.saturating_mul(dimensions.hash_operations))
				.saturating_add(
					weights.per_proof_byte.saturating_mul(dimensions.proof_bytes as u64),
				);
			assert_eq!(weights.router_variant(variant), exact);
		}
		assert_eq!(
			RouterWeightDimensionsV6::for_variant(RouterVariantV6::PersonalAliasAccountRevised)
				.writes,
			1
		);
		assert_eq!(
			RouterWeightDimensionsV6::for_variant(RouterVariantV6::LiteAliasAccountRevised).writes,
			1
		);
		let resources =
			RouterWeightDimensionsV6::for_variant(RouterVariantV6::ClaimLongTermStorage);
		assert_eq!((resources.reads, resources.writes, resources.crypto_verifications), (6, 0, 1));
		assert_eq!(
			(resources.hash_operations, resources.proof_bytes, resources.retained_revision_reads),
			(3, 32, 1)
		);
		let max_router = RouterVariantV6::ALL
			.into_iter()
			.map(|variant| weights.router_variant(variant))
			.max()
			.unwrap();
		assert_eq!(
			weights.malformed_classifier_and_router(),
			weights.maximum_rejection() + max_router
		);
	}

	#[test]
	fn token_is_one_shot_key_bound_and_empty_at_finalization() {
		let mut slot = PaidMetaTokenSlotV6::default();
		let value = slot.prepare_outer(token()).unwrap();
		assert_eq!(slot.prepare_outer(token()), Err(TokenError::Occupied));
		let mut wrong = value;
		wrong.outer_nonce += 1;
		assert_eq!(slot.consume_inner(wrong), Err(TokenError::KeyMismatch));
		let prepared = slot.consume_inner(value).unwrap();
		assert_eq!(prepared.payer, [7; 32]);
		assert_eq!(slot.consume_inner(value), Err(TokenError::Missing));
		assert_eq!(MetaTokenMustBeEmptyV3::assert(&slot), Ok(()));
		let value = slot.prepare_outer(token()).unwrap();
		assert_eq!(MetaTokenMustBeEmptyV3::assert(&slot), Err(TokenError::LeftoverAtFinalization));
		slot.cleanup_matching(H256::repeat_byte(1));
		assert_eq!(slot.post_transactions(), Err(TokenError::LeftoverAtFinalization));
		slot.cleanup_matching(value.token_key);
		assert_eq!(slot.post_transactions(), Ok(()));
		let mut consumed = token();
		consumed.consumed = true;
		let value = slot.prepare_outer(consumed).unwrap();
		assert_eq!(slot.consume_inner(value), Err(TokenError::AlreadyConsumed));
	}
}
