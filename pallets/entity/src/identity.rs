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
use core::fmt::Debug;
#[cfg(feature = "runtime-benchmarks")]
use enumflags2::BitFlag;
use enumflags2::{bitflags, BitFlags};
use frame_support::{traits::Get, CloneNoBound, EqNoBound, PartialEqNoBound, RuntimeDebugNoBound};
use scale_info::{build::Variants, Path, Type, TypeInfo};
use sp_runtime::BoundedVec;

use crate::types::{
	Additional, Attribute, Data, IdentityInformationProvider, IdentityUpdateError,
	IdentityUpdateOp, ProfileCid,
};
/// Each field corresponds to a field in the `IdentityInfo` struct.
#[bitflags]
#[repr(u64)]
#[derive(Clone, Copy, PartialEq, Eq, RuntimeDebugNoBound)]
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
#[scale_info(skip_type_params(FieldLimit, DataLimit))]
pub struct IdentityInfo<FieldLimit: Get<u32>, RawLimit: Get<u32>> {
	pub display: Data<RawLimit>,
	pub legal: Data<RawLimit>,
	pub web: Data<RawLimit>,
	pub profile: Option<ProfileCid>,
	pub additional: Additional<FieldLimit, RawLimit>,
}

impl<FieldLimit: Get<u32>, DataLimit: Get<u32>> IdentityInfo<FieldLimit, DataLimit> {
	pub fn set_additional(&mut self, new: BoundedVec<(Attribute, Data<DataLimit>), FieldLimit>) {
		self.additional = new;
	}

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

impl<
		FieldLimit: Get<u32> + 'static + TypeInfo,
		DataLimit: Get<u32> + 'static + TypeInfo + Clone + PartialEq + Debug + TypeInfo,
	> IdentityInformationProvider for IdentityInfo<FieldLimit, DataLimit>
{
	type FieldsIdentifier = u64;
	type FieldLimit = FieldLimit;
	type DataLimit = DataLimit;
	type UpdateOp = IdentityUpdateOp<DataLimit>;

	fn additional(&self) -> &Additional<Self::FieldLimit, Self::DataLimit> {
		&self.additional
	}

	fn has_identity(&self, fields: Self::FieldsIdentifier) -> bool {
		self.fields().bits() & fields == fields
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
			IdentityUpdateOp::SetProfile(o) => {
				self.profile = o.clone();
				Ok(())
			},
			IdentityUpdateOp::AddAdditional(k, v) => {
				if self.additional.iter().any(|(kk, _)| kk == k) {
					return Err(IdentityUpdateError::AttributeExists);
				}
				self.additional
					.try_push((k.clone(), v.clone()))
					.map_err(|_| IdentityUpdateError::TooManyAttributes)?;
				Ok(())
			},
			IdentityUpdateOp::UpdateAdditional(k, v) => {
				if let Some((_, val)) = self.additional.iter_mut().find(|(kk, _)| kk == k) {
					*val = v.clone();
					Ok(())
				} else {
					Err(IdentityUpdateError::AttributeNotFound)
				}
			},
			IdentityUpdateOp::RemoveAdditional(k) => {
				if let Some(i) = self.additional.iter().position(|(kk, _)| kk == k) {
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
		let empty = Data::<DataLimit>::Raw(Default::default());
		let mut all = Vec::new();
		let cap: usize = FieldLimit::get().try_into().unwrap();
		for _ in 0..cap {
			all.push((Attribute::default(), empty.clone()));
		}
		IdentityInfo {
			display: empty.clone(),
			legal: empty.clone(),
			web: empty.clone(),
			profile: Some(ProfileCid([0u8; 64])),
			additional: all.try_into().unwrap(),
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn all_fields() -> Self::FieldsIdentifier {
		IdentityField::all().bits()
	}
}

impl<FieldLimit: Get<u32>, RawLimit: Get<u32>> Default for IdentityInfo<FieldLimit, RawLimit> {
	fn default() -> Self {
		IdentityInfo {
			display: Data::None,
			legal: Data::None,
			web: Data::None,
			profile: None,
			additional: Default::default(),
		}
	}
}
