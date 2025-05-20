// This file is part of CORD – https://cord.network

// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.

use super::*;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use enumflags2::{bitflags, BitFlags};
use frame_support::{
	parameter_types, traits::Get, CloneNoBound, EqNoBound, PartialEqNoBound, RuntimeDebugNoBound,
};
use pallet_entity::types::{
	Attributes, Data, EntityInformationProvider, EntityUpdateError, EntityUpdateOp,
};
use scale_info::{build::Variants, Path, Type, TypeInfo};
use sp_runtime::RuntimeDebug;

parameter_types! {
	pub const MaxSubAccounts: u32 = 32;
	pub const MaxRawDataLength: u32 = 4096;
	pub const MaxUsernameLength: u32 = 32;
	pub const GeneralAdminBodyId: BodyId = BodyId::Administration;
}

pub type IdentityAdminOrigin = EitherOfDiverse<
	EnsureRoot<AccountId>,
	EnsureXcm<IsVoiceOfBody<GovernanceLocation, GeneralAdminBodyId>>,
>;

impl pallet_entity::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type MaxSubAccounts = MaxSubAccounts;
	type MaxRawDataLength = MaxRawDataLength;
	type EntityInformation = EntityInfo<MaxRawDataLength>;
	type MaxUsernameLength = MaxUsernameLength;
	type ForceOrigin = EnsureRoot<Self::AccountId>;
	type WeightInfo = weights::pallet_entity::WeightInfo<Runtime>;
}

/// Each field corresponds to a field in the `IdentityInfo` struct.
#[bitflags]
#[repr(u64)]
#[derive(Clone, Copy, PartialEq, Eq, RuntimeDebug)]
pub enum EntityField {
	Display,
	Legal,
	Web,
	Attributes,
}

impl TypeInfo for EntityField {
	type Identity = Self;

	fn type_info() -> scale_info::Type {
		Type::builder().path(Path::new("EntityField", module_path!())).variant(
			Variants::new()
				.variant("Display", |v| v.index(0))
				.variant("Legal", |v| v.index(1))
				.variant("Web", |v| v.index(2))
				.variant("Attributes", |v| v.index(3)),
		)
	}
}

/// Information concerning the identity of the controller of an account.
#[derive(
	CloneNoBound,
	Encode,
	Decode,
	DecodeWithMemTracking,
	EqNoBound,
	MaxEncodedLen,
	PartialEqNoBound,
	RuntimeDebugNoBound,
	TypeInfo,
)]
#[codec(mel_bound())]
#[scale_info(skip_type_params(MaxRawDataLength))]
pub struct EntityInfo<MaxRawDataLength: Get<u32>> {
	/// A reasonable display name (UTF-8).
	pub display: Data<MaxRawDataLength>,
	/// The full legal name (UTF-8).
	pub legal: Data<MaxRawDataLength>,
	/// A representative website (UTF-8, “https://” prepended).
	pub web: Data<MaxRawDataLength>,
	/// Additional arbitrary (key, value) pairs.
	pub attributes: Option<Attributes<MaxRawDataLength>>,
}

impl<MaxRawDataLength: Get<u32>> EntityInfo<MaxRawDataLength> {
	pub(crate) fn fields(&self) -> BitFlags<EntityField> {
		let mut bits = BitFlags::empty();
		if !self.display.is_none() {
			bits.insert(EntityField::Display);
		}
		if !self.legal.is_none() {
			bits.insert(EntityField::Legal);
		}
		if !self.web.is_none() {
			bits.insert(EntityField::Web);
		}
		if let Some(attrs) = &self.attributes {
			if !attrs.is_empty() {
				bits.insert(EntityField::Attributes);
			}
		}
		bits
	}
}

impl<MaxRawDataLength: Get<u32> + 'static> EntityInformationProvider
	for EntityInfo<MaxRawDataLength>
{
	type FieldsIdentifier = u64;
	type MaxRawDataLength = MaxRawDataLength;
	type UpdateOp = EntityUpdateOp<MaxRawDataLength>;

	fn attributes(&self) -> Option<&Attributes<Self::MaxRawDataLength>> {
		self.attributes.as_ref()
	}

	fn get_key(&self, key: &[u8]) -> Data<Self::MaxRawDataLength> {
		match key {
			b"display" => self.display.clone(),
			b"legal" => self.legal.clone(),
			b"web" => self.web.clone(),
			_ => self
				.attributes
				.as_ref()
				.and_then(|attrs| {
					attrs
						.iter()
						// compare the raw byte‐slices
						.find(|(k, _)| &k[..] == &key[..])
						.map(|(_, v)| v.clone())
				})
				.unwrap_or(Data::None),
		}
	}

	fn present_fields(&self) -> Self::FieldsIdentifier {
		self.fields().bits()
	}

	fn has_info_fields(&self, fields: Self::FieldsIdentifier) -> bool {
		self.present_fields() & fields == fields
	}

	fn apply_update(&mut self, op: &Self::UpdateOp) -> Result<(), EntityUpdateError> {
		match op {
			&EntityUpdateOp::SetKey(ref key, ref val) => {
				let key_bytes: &[u8] = &key[..];
				if key_bytes == b"display" {
					self.display = val.clone();
				} else if key_bytes == b"legal" {
					self.legal = val.clone();
				} else if key_bytes == b"web" {
					self.web = val.clone();
				} else {
					let attrs = self.attributes.get_or_insert_with(Default::default);
					if let Some((_, _)) = attrs.iter().find(|(k, _)| k == key) {
						for &mut (ref mut kk, ref mut vv) in attrs.iter_mut() {
							if kk == key {
								*vv = val.clone();
								return Ok(());
							}
						}
					} else {
						attrs
							.try_push((key.clone(), val.clone()))
							.map_err(|_| EntityUpdateError::TooManyAttributes)?;
					}
				}

				Ok(())
			},

			&EntityUpdateOp::RemoveKey(ref key) => {
				let key_bytes: &[u8] = &key[..];
				if key_bytes == b"display" {
					self.display = Data::None;
				} else if key_bytes == b"legal" {
					self.legal = Data::None;
				} else if key_bytes == b"web" {
					self.web = Data::None;
				} else {
					if let Some(ref mut attrs) = self.attributes {
						if let Some(idx) = attrs.iter().position(|(k, _)| k == key) {
							attrs.swap_remove(idx);
							if attrs.is_empty() {
								self.attributes = None;
							}
						} else {
							return Err(EntityUpdateError::AttributeNotFound);
						}
					} else {
						return Err(EntityUpdateError::AttributeNotFound);
					}
				}

				Ok(())
			},

			&EntityUpdateOp::ClearAll => {
				self.display = Data::None;
				self.legal = Data::None;
				self.web = Data::None;
				self.attributes = None;
				Ok(())
			},
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn create_entity_info() -> Self {
		let empty = Data::<MaxRawDataLength>::Raw(Default::default());
		let mut all = Vec::new();
		let cap: usize = FieldLimit::get().try_into().unwrap();
		for _ in 0..cap {
			all.push((Attribute::default(), empty.clone()));
		}
		EntityInfo {
			display: empty.clone(),
			legal: empty.clone(),
			web: empty.clone(),
			attributes: all.try_into().unwrap(),
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn all_fields() -> Self::FieldsIdentifier {
		EntityField::all().bits()
	}
}

impl<MaxRawDataLength: Get<u32>> Default for EntityInfo<MaxRawDataLength> {
	fn default() -> Self {
		EntityInfo {
			display: Data::<MaxRawDataLength>::None,
			legal: Data::<MaxRawDataLength>::None,
			web: Data::<MaxRawDataLength>::None,
			attributes: None,
		}
	}
}
