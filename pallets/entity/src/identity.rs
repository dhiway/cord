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

#[cfg(feature = "runtime-benchmarks")]
use alloc::vec;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
#[cfg(feature = "runtime-benchmarks")]
use enumflags2::BitFlag;
use enumflags2::{bitflags, BitFlags};
use frame_support::{traits::Get, CloneNoBound, EqNoBound, PartialEqNoBound, RuntimeDebugNoBound};
use scale_info::{build::Variants, Path, Type, TypeInfo};
use sp_runtime::{BoundedVec, RuntimeDebug};

use crate::types::{
	Attribute, Data, IdentityInformationProvider, IdentityUpdateError, IdentityUpdateOp, ProfileCid,
};
/// The fields that we use to identify the owner of an account with. Each corresponds to a field
/// in the `IdentityInfo` struct.
#[bitflags]
#[repr(u64)]
#[derive(Clone, Copy, PartialEq, Eq, RuntimeDebug)]
pub enum IdentityField {
	Display,
	Legal,
	Web,
	Profile,
	Additional,
}

impl TypeInfo for IdentityField {
	type Identity = Self;

	fn type_info() -> scale_info::Type {
		Type::builder().path(Path::new("IdentityField", module_path!())).variant(
			Variants::new()
				.variant("Display", |v| v.index(0))
				.variant("Legal", |v| v.index(1))
				.variant("Web", |v| v.index(2))
				.variant("Profile", |v| v.index(3))
				.variant("Additional", |v| v.index(4)),
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
#[scale_info(skip_type_params(FieldLimit))]
pub struct IdentityInfo<FieldLimit: Get<u32>> {
	/// A reasonable display name (UTF-8).
	pub display: Data,
	/// The full legal name (UTF-8).
	pub legal: Data,
	/// A representative website (UTF-8, “https://” prepended).
	pub web: Data,
	/// A content identifier (CID) for a profile blob or document.
	pub profile: Option<ProfileCid>,
	/// Additional arbitrary (key, value) pairs.
	pub additional: BoundedVec<(Attribute, Data), FieldLimit>,
}

impl<FieldLimit: Get<u32>> IdentityInfo<FieldLimit> {
	pub fn set_additional(&mut self, add: BoundedVec<(Attribute, Data), FieldLimit>) {
		self.additional = add;
	}
}

impl<FieldLimit: Get<u32> + 'static> IdentityInformationProvider for IdentityInfo<FieldLimit> {
	type FieldsIdentifier = u64;
	type FieldLimit = FieldLimit;
	type UpdateOp = IdentityUpdateOp;

	fn has_identity(&self, fields: Self::FieldsIdentifier) -> bool {
		self.fields().bits() & fields == fields
	}

	fn additional(&self) -> &BoundedVec<(Attribute, Data), FieldLimit> {
		&self.additional
	}

	fn apply_update(&mut self, op: &Self::UpdateOp) -> Result<(), IdentityUpdateError> {
		match op {
			IdentityUpdateOp::SetDisplay(x) => {
				self.display = x.clone();
				Ok(())
			},
			IdentityUpdateOp::SetLegal(x) => {
				self.legal = x.clone();
				Ok(())
			},
			IdentityUpdateOp::SetWeb(x) => {
				self.web = x.clone();
				Ok(())
			},
			IdentityUpdateOp::SetProfile(opt) => {
				self.profile = opt.clone();
				Ok(())
			},

			IdentityUpdateOp::AddAdditional(key, val) => {
				if self.additional.iter().any(|(k, _)| k == key) {
					return Err(IdentityUpdateError::AttributeExists);
				}
				self.additional
					.try_push((key.clone(), val.clone()))
					.map_err(|_| IdentityUpdateError::TooManyAttributes)?;
				Ok(())
			},

			IdentityUpdateOp::UpdateAdditional(key, val) => {
				if let Some((_, v)) = self.additional.iter_mut().find(|(k, _)| k == key) {
					*v = val.clone();
					Ok(())
				} else {
					Err(IdentityUpdateError::AttributeNotFound)
				}
			},

			IdentityUpdateOp::RemoveAdditional(key) => {
				if let Some(i) = self.additional.iter().position(|(k, _)| k == key) {
					self.additional.swap_remove(i);
					Ok(())
				} else {
					Err(IdentityUpdateError::AttributeNotFound)
				}
			},

			IdentityUpdateOp::ClearAdditional => {
				self.additional.clear();
				Ok(())
			},
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn create_identity_info() -> Self {
		let d = Data::Raw(vec![0; 32].try_into().unwrap());
		let mut additional = Vec::new();
		let cap: usize = FieldLimit::get().try_into().unwrap();
		for _ in 0..cap {
			additional.push((AdditionalKey::default(), raw.clone()));
		}

		IdentityInfo {
			display: d.clone(),
			legal: d.clone(),
			web: d.clone(),
			profile: Some(ProfileCid([0u8; 64])),
			additional: additional.try_into().unwrap(),
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn all_fields() -> Self::FieldsIdentifier {
		IdentityField::all().bits()
	}
}

impl<FieldLimit: Get<u32>> Default for IdentityInfo<FieldLimit> {
	fn default() -> Self {
		IdentityInfo {
			display: Data::None,
			legal: Data::None,
			web: Data::None,
			profile: None,
			additional: BoundedVec::default(),
		}
	}
}

impl<FieldLimit: Get<u32>> IdentityInfo<FieldLimit> {
	pub(crate) fn fields(&self) -> BitFlags<IdentityField> {
		let mut bits = BitFlags::empty();
		if !self.display.is_none() {
			bits.insert(IdentityField::Display);
		}
		if !self.legal.is_none() {
			bits.insert(IdentityField::Legal);
		}
		if !self.web.is_none() {
			bits.insert(IdentityField::Web);
		}
		if self.profile.is_some() {
			bits.insert(IdentityField::Profile);
		}
		if !self.additional.is_empty() {
			bits.insert(IdentityField::Additional);
		}
		bits
	}
}
