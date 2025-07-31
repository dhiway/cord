use bitflags::bitflags;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
pub use cord_primitives::doket::{
	Attribute, Attributes, DoketInformationProvider, DoketUpdateError, DoketUpdateOp, Element,
};
use cord_primitives::identifier::Ss58Identifier;
use frame_support::{ensure, traits::Get, RuntimeDebugNoBound};
use scale_info::TypeInfo;

// Permissions for registry management.
bitflags! {
	#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, DecodeWithMemTracking)]
	pub struct Permissions: u16 {
		const ENTRY    = 1 << 0;
		const DELEGATE = 1 << 1;
		const REVOKE   = 1 << 2;
		const ADMIN    = 1 << 3;
	}
}

impl Default for Permissions {
	fn default() -> Self {
		Permissions::ENTRY
	}
}

impl Permissions {
	/// Construct a bitmask from a slice of variants.
	pub fn from_list(list: &[Permissions]) -> Self {
		list.iter().copied().fold(Self::empty(), |acc, p| acc | p)
	}
	/// Check if `ENTRY` permission is set.
	pub fn has_entry(self) -> bool {
		self.contains(Permissions::ADMIN) || self.contains(Permissions::ENTRY)
	}
	/// Check if `DELEGATE` permission is set.
	pub fn has_delegate(self) -> bool {
		self.contains(Permissions::ADMIN) || self.contains(Permissions::DELEGATE)
	}
	/// Check if `REVOKE` permission is set.
	pub fn has_revoke(self) -> bool {
		self.contains(Permissions::ADMIN) || self.contains(Permissions::REVOKE)
	}
	/// Check if `ADMIN` permission is set.
	pub fn has_admin(self) -> bool {
		self.contains(Permissions::ADMIN)
	}
	/// Add a permission to the mask.
	pub fn add_permission(&mut self, perm: Permissions) {
		*self |= perm;
	}
	/// Remove a permission from the mask.
	pub fn remove_permission(&mut self, perm: Permissions) {
		*self &= !perm;
	}
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebugNoBound, TypeInfo, MaxEncodedLen)]
pub enum RegisterAccess {
	TxFee,
	PayPerQuery,
	Subscription,
}
impl Default for RegisterAccess {
	fn default() -> Self {
		RegisterAccess::TxFee
	}
}

impl RegisterAccess {
	fn as_bytes(&self) -> &'static [u8] {
		match self {
			RegisterAccess::TxFee => b"tx_fee",
			RegisterAccess::PayPerQuery => b"pay_per_query",
			RegisterAccess::Subscription => b"subscription",
		}
	}

	fn from_bytes(key: &[u8]) -> Option<Self> {
		match key {
			b"tx_fee" => Some(RegisterAccess::TxFee),
			b"pay_per_query" => Some(RegisterAccess::PayPerQuery),
			b"subscription" => Some(RegisterAccess::Subscription),
			_ => None,
		}
	}
}

bitflags! {
	#[derive(Encode, Decode,  MaxEncodedLen, DecodeWithMemTracking)]
	pub struct RegisterField: u64 {
		const INFO        = 1 << 0;
		const MAINTAINER  = 1 << 1;
		const ATTRIBUTES  = 1 << 2;
	}
}

const REGISTER_RESERVED_KEYS: &[(RegisterField, &'static [u8])] = &[
	(RegisterField::INFO, b"info"),
	(RegisterField::MAINTAINER, b"maintainer"),
	(RegisterField::ATTRIBUTES, b"attributes"),
];

impl RegisterField {
	/// Map a reserved key name to its enum variant, or `None` if it is not one of these five.
	pub fn from_bytes(key: &[u8]) -> Option<Self> {
		REGISTER_RESERVED_KEYS
			.iter()
			.find(|(_, name)| *name == key)
			.map(|(field, _)| *field)
	}
	/// Convert the flag to its byte‐string representation.
	pub fn to_bytes(self) -> &'static [u8] {
		REGISTER_RESERVED_KEYS
			.iter()
			.find(|(field, _)| *field == self)
			.map(|(_, name)| *name)
			.unwrap_or(&[])
	}
}

impl TypeInfo for RegisterField {
	type Identity = Self;
	fn type_info() -> Type {
		Type::builder().path(Path::new("RegisterField", module_path!())).variant(
			Variants::new()
				.variant("Info", |v| v.index(0))
				.variant("Maintainer", |v| v.index(1))
				.variant("Attributes", |v| v.index(2)),
		)
	}
}

// Core registry information.
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
#[scale_info(skip_type_params(MaxRawDataLength, MaxAdditionalAttributes))]
pub struct RegisterInfo<MaxRawDataLength: Get<u32>, MaxAdditionalAttributes: Get<u32>> {
	/// Combined “info blob” (e.g. name + description)
	pub info: Element<MaxRawDataLength>,
	/// SS58‐ID of the registry’s maintainer
	pub maintainer: Element<MaxRawDataLength>,
	/// Any other dynamic `(Attribute → Element)` pairs
	pub attributes: Option<Attributes<MaxRawDataLength, MaxAdditionalAttributes>>,
}

impl<MaxRawDataLength: Get<u32>, MaxAdditionalAttributes: Get<u32>>
	RegisterInfo<MaxRawDataLength, MaxAdditionalAttributes>
{
	pub fn fields_mask(&self) -> RegisterField {
		let mut bits = RegisterField::empty();
		if !self.info.is_none() {
			bits |= RegisterField::INFO;
		}
		if !self.maintainer().is_none() {
			bits |= RegisterField::MAINTAINER;
		}
		if let Some(attrs) = &self.attributes {
			if !attrs.is_empty() {
				bits |= RegisterField::ATTRIBUTES;
			}
		}
		bits
	}
}

impl<MaxRawDataLength: Get<u32>, MaxAdditionalAttributes: Get<u32>> Default
	for RegisterInfo<MaxRawDataLength, MaxAdditionalAttributes>
{
	fn default() -> Self {
		RegisterInfo { info: Element::default(), maintainer: Element::default(), attributes: None }
	}
}

impl<MaxRawDataLength: Get<u32>, MaxAdditionalAttributes: Get<u32>>
	RegisterInfo<MaxRawDataLength, MaxAdditionalAttributes>
{
	/// Update the “info blob”
	pub fn set_info(&mut self, blob: Element<MaxRawDataLength>) {
		self.info = blob;
	}
	pub fn info_blob(&self) -> &Element<MaxRawDataLength> {
		&self.info
	}

	pub fn set_maintainer(&mut self, who: Ss58Identifier) {
		self.maintainer = Element::Doken(who);
	}

	pub fn maintainer(&self) -> Option<&Ss58Identifier> {
		if let Element::Doken(ref id) = self.maintainer {
			Some(id)
		} else {
			None
		}
	}

	pub fn validate_attributes(&self) -> Result<(), DoketUpdateError> {
		if let Some(attrs) = &self.attributes {
			let mut seen = Vec::with_capacity(attrs.len());
			for (key, _) in attrs.iter() {
				let raw = key.as_ref();
				if raw.is_empty() {
					return Err(DoketUpdateError::AttributeNotFound);
				}
				if RegisterField::from_bytes(raw).is_some() {
					return Err(DoketUpdateError::ReservedAttribute);
				}
				if seen.contains(&raw) {
					return Err(DoketUpdateError::AttributeExists);
				}
				seen.push(raw);
			}
		}
		Ok(())
	}

	pub fn add_attribute(
		&mut self,
		key: Attribute,
		value: Element<MaxRawDataLength>,
	) -> Result<(), DoketUpdateError> {
		let attrs = self.attributes.get_or_insert_with(Default::default);
		ensure!(!attrs.iter().any(|(k, _)| k == &key), DoketUpdateError::AttributeExists);
		attrs.try_push((key, value)).map_err(|_| DoketUpdateError::TooManyAttributes)
	}

	pub fn remove_attribute(&mut self, key: &Attribute) -> Result<(), DoketUpdateError> {
		let attrs = self.attributes.as_mut().ok_or(DoketUpdateError::AttributeNotFound)?;
		let idx = attrs
			.iter()
			.position(|(k, _)| k == key)
			.ok_or(DoketUpdateError::AttributeNotFound)?;
		attrs.swap_remove(idx);
		if attrs.is_empty() {
			self.attributes = None;
		}
		Ok(())
	}

	pub fn get_attribute(&self, key: &[u8]) -> Option<Element<MaxRawDataLength>> {
		self.attributes.as_ref().and_then(|attrs| {
			attrs.iter().find(|(k, _)| k.as_slice() == key).map(|(_, v)| v.clone())
		})
	}

	pub fn attribute_keys(&self) -> Vec<Vec<u8>> {
		self.attributes
			.as_ref()
			.map_or_else(Vec::new, |attrs| attrs.iter().map(|(k, _)| k.to_vec()).collect())
	}
}

impl<MaxRawDataLength: Get<u32> + 'static, MaxAdditionalAttributes: Get<u32>>
	DoketInformationProvider for RegisterInfo<MaxRawDataLength, MaxAdditionalAttributes>
{
	type FieldMask = u64;
	type MaxRawDataLength = MaxRawDataLength;
	type MaxAdditionalAttributes = MaxAdditionalAttributes;
	type UpdateOp = DoketUpdateOp<MaxRawDataLength>;

	fn create_info() -> Self {
		Default::default()
	}

	fn all_fields() -> Self::FieldMask {
		RegisterField::all().bits()
	}

	fn attributes(
		&self,
	) -> Option<&Attributes<Self::MaxRawDataLength, Self::MaxAdditionalAttributes>> {
		self.attributes.as_ref()
	}

	fn get_key(&self, key: &[u8]) -> Element<Self::MaxRawDataLength> {
		if let Some(field) = RegisterField::from_bytes(key) {
			return match field {
				RegisterField::INFO => self.info.clone(),
				RegisterField::MAINTAINER => self.maintainer.clone(),
				RegisterField::ATTRIBUTES => Element::None,
				_ => Element::None,
			};
		}

		self.get_attribute(key).unwrap_or_default()
	}

	fn present_fields(&self) -> Self::FieldMask {
		self.fields_mask().bits()
	}

	fn has_info_fields(&self, mask: Self::FieldMask) -> bool {
		(self.present_fields() & mask) == mask
	}

	fn apply_update(&mut self, op: &Self::UpdateOp) -> Result<(), DoketUpdateError> {
		match op {
			DoketUpdateOp::AddAttribute(k, v) => {
				ensure!(
					RegisterField::from_bytes(k.as_ref()).is_none(),
					DoketUpdateError::ReservedAttribute
				);
				self.add_attribute(k.clone(), v.clone())
			},
			DoketUpdateOp::RemoveAttribute(k) => {
				ensure!(
					RegisterField::from_bytes(k.as_ref()).is_none(),
					DoketUpdateError::ReservedAttribute
				);

				let key: Attribute = k.clone();
				self.remove_attribute(&key)
			},
			DoketUpdateOp::UpdateAttribute(k, v) => {
				if let Some(field) = RegisterField::from_bytes(k.as_ref()) {
					match field {
						RegisterField::INFO => {
							self.info = v.clone();
							Ok(())
						},
						RegisterField::MAINTAINER => match v {
							Element::Doken(ref new_id) => {
								self.maintainer = Element::Doken(new_id.clone());
								Ok(())
							},
							_ => Err(DoketUpdateError::Invalididentifier),
						},
						_ => Err(DoketUpdateError::AttributeNotFound),
					}
				} else {
					let key: Attribute = k.clone();
					let attrs =
						self.attributes.as_mut().ok_or(DoketUpdateError::AttributeNotFound)?;
					if let Some((_, existing)) = attrs.iter_mut().find(|(kk, _)| kk == &key) {
						*existing = v.clone();
						Ok(())
					} else {
						Err(DoketUpdateError::AttributeNotFound)
					}
				}
			},
		}
	}
}
