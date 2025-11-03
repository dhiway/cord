use alloc::{collections::BTreeSet, vec::Vec};
use bitflags::bitflags;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use cord_primitives::{
	identifier::Ss58Identifier,
	packet::{Attribute, Element, ElementType, PacketUpdateError},
};
use frame_support::{ensure, traits::Get, BoundedVec, RuntimeDebugNoBound};
use scale_info::TypeInfo;

bitflags! {
	#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, DecodeWithMemTracking)]
	pub struct RegistryPermissions: u16 {
		const VIEW     = 1 << 3;
		const ENTRY    = 1 << 0;
		const DELEGATE = 1 << 1;
		const ADMIN    = 1 << 2;
	}
}

impl Default for RegistryPermissions {
	fn default() -> Self {
		RegistryPermissions::ENTRY | RegistryPermissions::VIEW
	}
}

impl RegistryPermissions {
	pub fn from_list(list: &[RegistryPermissions]) -> Self {
		let mask = list.iter().copied().fold(Self::empty(), |acc, p| acc | p);
		mask.with_implied_view()
	}

	pub fn with_implied_view(mut self) -> Self {
		if self.contains(RegistryPermissions::ADMIN) || self.contains(RegistryPermissions::ENTRY) {
			self |= RegistryPermissions::VIEW;
		}
		self
	}

	pub fn has_entry(self) -> bool {
		self.contains(RegistryPermissions::ADMIN) || self.contains(RegistryPermissions::ENTRY)
	}

	pub fn has_delegate(self) -> bool {
		self.contains(RegistryPermissions::ADMIN) || self.contains(RegistryPermissions::DELEGATE)
	}

	pub fn has_admin(self) -> bool {
		self.contains(RegistryPermissions::ADMIN)
	}

	pub fn has_view(self) -> bool {
		self.contains(RegistryPermissions::VIEW)
			|| self.contains(RegistryPermissions::ENTRY)
			|| self.contains(RegistryPermissions::ADMIN)
	}
}

#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	Clone,
	PartialEq,
	Eq,
	RuntimeDebugNoBound,
	TypeInfo,
	MaxEncodedLen,
	Default,
)]
pub enum RegistryKind {
	#[default]
	Raw,
	Token,
	Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegistryFieldError {
	DuplicateKey,
	UnknownKey,
	EmptySpec,
	DuplicateSpec,
}

/// Lookup specification describing how attribute keys are reused without duplicating values.
#[derive(Encode, Decode, DecodeWithMemTracking, RuntimeDebugNoBound, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(MaxAdditionalAttributes))]
pub enum LookupSpec<MaxAdditionalAttributes: Get<u32>> {
	/// Single attribute key reused for token or lookup material.
	Single(Attribute),
	/// Bounded list of attribute keys representing a composite index.
	Combo(BoundedVec<Attribute, MaxAdditionalAttributes>),
}

impl<MaxAdditionalAttributes: Get<u32>> Clone for LookupSpec<MaxAdditionalAttributes> {
	fn clone(&self) -> Self {
		match self {
			Self::Single(attr) => Self::Single(attr.clone()),
			Self::Combo(list) => Self::Combo(list.clone()),
		}
	}
}

impl<MaxAdditionalAttributes: Get<u32>> PartialEq for LookupSpec<MaxAdditionalAttributes> {
	fn eq(&self, other: &Self) -> bool {
		match (self, other) {
			(Self::Single(lhs), Self::Single(rhs)) => lhs == rhs,
			(Self::Combo(lhs), Self::Combo(rhs)) => lhs == rhs,
			_ => false,
		}
	}
}

impl<MaxAdditionalAttributes: Get<u32>> Eq for LookupSpec<MaxAdditionalAttributes> {}

impl<MaxAdditionalAttributes: Get<u32>> LookupSpec<MaxAdditionalAttributes> {
	pub fn key_count(&self) -> usize {
		match self {
			Self::Single(_) => 1,
			Self::Combo(list) => list.len(),
		}
	}

	pub fn total_key_bytes(&self) -> usize {
		match self {
			Self::Single(attr) => attr.len(),
			Self::Combo(list) => list.iter().map(|attr| attr.len()).sum(),
		}
	}

	pub fn cloned_keys(&self) -> Vec<Attribute> {
		match self {
			Self::Single(attr) => vec![attr.clone()],
			Self::Combo(list) => list.iter().cloned().collect(),
		}
	}

	pub fn fingerprint(&self) -> Vec<u8> {
		let mut normalized: Vec<Vec<u8>> =
			self.cloned_keys().into_iter().map(|attr| attr.to_vec()).collect();
		normalized.sort();
		normalized.encode()
	}
}

/// Core packet held in storage for each registry.
#[derive(
	Encode,
	Decode,
	Clone,
	PartialEq,
	Eq,
	RuntimeDebugNoBound,
	TypeInfo,
	MaxEncodedLen,
	DecodeWithMemTracking,
)]
#[scale_info(skip_type_params(MaxRawDataLength, MaxAdditionalAttributes,))]
pub struct RegistryInfo<MaxRawDataLength: Get<u32>, MaxAdditionalAttributes: Get<u32>> {
	/// Arbitrary registry description / metadata blob.
	pub info: Element<MaxRawDataLength>,
	/// Token of the maintainer (registry owner).
	pub maintainer: Ss58Identifier,
	/// Attribute schema (key → expected value type). Keys MUST be unique.
	pub attributes: BoundedVec<(Attribute, ElementType), MaxAdditionalAttributes>,
	/// Attribute key combination used to derive the registry token material.
	pub token_spec: LookupSpec<MaxAdditionalAttributes>,
	/// One or more attribute-key combinations that serve as lookup specs.
	pub lookup_specs: BoundedVec<LookupSpec<MaxAdditionalAttributes>, MaxAdditionalAttributes>,
	/// Registry kind (Raw/Token/Hash).
	pub kind: RegistryKind,
	/// Registry status flag.
	pub is_active: bool,
}

impl<MaxRawDataLength: Get<u32>, MaxAdditionalAttributes: Get<u32>>
	RegistryInfo<MaxRawDataLength, MaxAdditionalAttributes>
{
	/// Set the registry info blob.
	pub fn set_info(&mut self, blob: Element<MaxRawDataLength>) {
		self.info = blob;
	}

	/// Set the maintainer (owner) token. **Non-optional**.
	pub fn set_maintainer(&mut self, maintainer: Ss58Identifier) {
		self.maintainer = maintainer;
	}

	/// Borrow the maintainer token.
	pub fn maintainer(&self) -> &Ss58Identifier {
		&self.maintainer
	}

	/// Set the registry kind.
	pub fn set_kind(&mut self, kind: RegistryKind) {
		self.kind = kind;
	}

	/// Toggle active/inactive status.
	pub fn set_status(&mut self, is_active: bool) {
		self.is_active = is_active;
	}

	/// Replace the full attribute list after validation.
	pub fn set_attributes(
		&mut self,
		attributes: BoundedVec<(Attribute, ElementType), MaxAdditionalAttributes>,
	) -> Result<(), PacketUpdateError> {
		Self::ensure_valid_attributes(&attributes)?;
		self.attributes = attributes;
		Ok(())
	}

	/// Internal helper to validate a deduplicated subset of attribute keys.
	fn ensure_key_subset(&self, fields: &[Attribute]) -> Result<(), RegistryFieldError> {
		let mut seen = BTreeSet::new();
		let attribute_keys: BTreeSet<&[u8]> =
			self.attributes.iter().map(|(key, _)| key.as_slice()).collect();

		for key in fields.iter() {
			let inserted = seen.insert(key.as_slice());
			if !inserted {
				return Err(RegistryFieldError::DuplicateKey);
			}
			if !attribute_keys.contains(key.as_slice()) {
				return Err(RegistryFieldError::UnknownKey);
			}
		}
		Ok(())
	}

	/// Ensure that every lookup spec references known attributes and is unique.
	fn ensure_lookup_specs(
		&self,
		specs: &[LookupSpec<MaxAdditionalAttributes>],
	) -> Result<(), RegistryFieldError> {
		let mut seen_specs = BTreeSet::new();
		for spec in specs.iter() {
			let keys = spec.cloned_keys();
			if keys.is_empty() {
				return Err(RegistryFieldError::EmptySpec);
			}
			self.ensure_key_subset(&keys)?;
			let fingerprint = spec.fingerprint();
			if !seen_specs.insert(fingerprint) {
				return Err(RegistryFieldError::DuplicateSpec);
			}
		}
		Ok(())
	}

	/// Define the attribute keys used to derive the token.
	pub fn set_token_spec(
		&mut self,
		spec: LookupSpec<MaxAdditionalAttributes>,
	) -> Result<(), RegistryFieldError> {
		let keys = spec.cloned_keys();
		if keys.is_empty() {
			return Err(RegistryFieldError::EmptySpec);
		}
		self.ensure_key_subset(&keys)?;
		self.token_spec = spec;
		Ok(())
	}

	/// Define the attribute-key combinations that will be used for indexed lookups.
	pub fn set_lookup_specs(
		&mut self,
		specs: BoundedVec<LookupSpec<MaxAdditionalAttributes>, MaxAdditionalAttributes>,
	) -> Result<(), RegistryFieldError> {
		self.ensure_lookup_specs(&specs)?;
		self.lookup_specs = specs;
		Ok(())
	}

	/// Get a cloned attribute value by key.
	pub fn attribute_type(&self, key: &[u8]) -> Option<ElementType> {
		self.attributes
			.iter()
			.find(|(attr, _)| attr.as_slice() == key)
			.map(|(_, value)| *value)
	}

	/// Get all attribute keys (unordered).
	pub fn attribute_keys(&self) -> Vec<Vec<u8>> {
		self.attributes.iter().map(|(k, _)| k.to_vec()).collect()
	}

	/// Resolve key/value pairs for the token, in the key order defined by `token_spec`.
	pub fn resolve_token_material(&self) -> Vec<(Attribute, ElementType)> {
		self.token_spec
			.cloned_keys()
			.into_iter()
			.map(|key| {
				let value = self
					.attributes
					.iter()
					.find(|(attr, _)| attr.as_slice() == key.as_slice())
					.expect("token field subset validated during creation")
					.1;
				(key, value)
			})
			.collect()
	}

	/// Validate attribute list invariants:
	fn ensure_valid_attributes(
		attributes: &[(Attribute, ElementType)],
	) -> Result<(), PacketUpdateError> {
		let mut seen = BTreeSet::new();
		for (key, _) in attributes.iter() {
			let raw = key.as_slice();
			ensure!(!raw.is_empty(), PacketUpdateError::AttributeNotFound);
			if !seen.insert(raw) {
				return Err(PacketUpdateError::AttributeExists);
			}
		}
		Ok(())
	}
}
