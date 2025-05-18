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
use cord_primitives::element::Element;
use core::fmt::Debug;
use frame_support::{
	traits::{ConstU32, Get},
	BoundedVec, CloneNoBound, RuntimeDebugNoBound,
};
use scale_info::TypeInfo;
use sp_runtime::RuntimeDebug;

/// The raw‐data type used throughout the identity pallet.
// pub type Data<MaxRaw: Get<u32>> = Element<MaxRaw>;
pub type Data<MaxRaw> = Element<MaxRaw>;

/// Maximum length for an additional-field key.
pub type Attribute = BoundedVec<u8, ConstU32<64>>;

// pub type Additional<MaxFields: Get<u32>, MaxRaw: Get<u32>> =
pub type Additional<MaxFields, MaxRaw> = BoundedVec<(Attribute, Data<MaxRaw>), MaxFields>;

/// A `Data::CID`‐only wrapper; trying to build it from any other `Data` will fail.
#[derive(
	Clone,
	Encode,
	Decode,
	DecodeWithMemTracking,
	PartialEq,
	Eq,
	RuntimeDebug,
	TypeInfo,
	MaxEncodedLen,
)]
pub struct ProfileCid(pub [u8; 64]);

/// Convert our wrapper into a full `Data::CID(_)`.
impl<MaxRaw: Get<u32>> From<ProfileCid> for Data<MaxRaw> {
	fn from(cid: ProfileCid) -> Self {
		Data::CID(cid.0)
	}
}

/// Try to extract a `ProfileCid` from a general `Data`
impl<MaxRaw: Get<u32>> TryFrom<Data<MaxRaw>> for ProfileCid {
	type Error = ();
	fn try_from(value: Data<MaxRaw>) -> Result<Self, ()> {
		match value {
			Data::CID(bytes) => Ok(ProfileCid(bytes)),
			_ => Err(()),
		}
	}
}

/// Errors that can occur when applying a single `IdentityUpdateOp`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentityUpdateError {
	/// Tried to add a key that already exists.
	AttributeExists,
	/// Exceeded the maximum number of additional key/value pairs.
	TooManyAttributes,
	/// Tried to update or remove a key that doesn't exist.
	AttributeNotFound,
}

/// Information concerning the identity of the controller of an account.
pub trait IdentityInformationProvider:
	Encode + Decode + MaxEncodedLen + Clone + Debug + Eq + PartialEq + TypeInfo + Default
{
	/// Bitmask type for which fields are set/updated.
	type FieldsIdentifier: Encode + Decode + MaxEncodedLen + TypeInfo + Default;

	/// Limit on the number of `(Attribute, Data)` pairs.
	type FieldLimit: Get<u32>;

	/// Limit on the raw‐data size.
	type DataLimit: Get<u32>;

	/// The enum of update operations.
	type UpdateOp: Encode + Decode + Clone + Debug + PartialEq + TypeInfo + MaxEncodedLen;

	/// Access the current additional entries (never `None`).
	fn additional(&self) -> &Additional<Self::FieldLimit, Self::DataLimit>;

	/// Do we have *all* the requested data fields?
	fn has_identity(&self, fields: Self::FieldsIdentifier) -> bool;

	/// Apply one operation.
	fn apply_update(&mut self, op: &Self::UpdateOp) -> Result<(), IdentityUpdateError>;

	/// For benchmarking only.
	#[cfg(feature = "runtime-benchmarks")]
	fn create_identity_info() -> Self;

	/// For benchmarking only.
	#[cfg(feature = "runtime-benchmarks")]
	fn all_fields() -> Self::FieldsIdentifier;
}

/// An atomic identity update operation.
#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	CloneNoBound,
	PartialEq,
	Eq,
	RuntimeDebugNoBound,
	TypeInfo,
	MaxEncodedLen,
)]
// #[scale_info(skip_type_params(MaxRaw))]
pub enum IdentityUpdateOp<MaxRaw: Get<u32>> {
	/// Replace the display name.
	SetDisplay(Data<MaxRaw>),
	/// Replace the legal name.
	SetLegal(Data<MaxRaw>),
	/// Replace the website.
	SetWeb(Data<MaxRaw>),
	/// Replace the profile CID (or clear if `None`).
	SetProfile(Option<ProfileCid>),
	/// Add a new key/value pair. Fails if key exists or limit reached.
	AddAdditional(Attribute, Data<MaxRaw>),
	/// Update an existing key’s value (or clear if `Data::None`).
	UpdateAdditional(Attribute, Data<MaxRaw>),
	/// Remove a key/value pair by key.
	RemoveAdditional(Attribute),
	/// Remove *all* additional data.
	ClearAdditional,
}

/// A byte vec used to represent a username.
pub(crate) type Username<T> = BoundedVec<u8, <T as Config>::MaxUsernameLength>;

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_data_roundtrip_and_as_ref() {
		// None variant
		type TestData = Data<ConstU32<128>>;
		let d_none = TestData::None;
		let enc_none = d_none.encode();
		let dec_none = TestData::decode(&mut &enc_none[..]).expect("Decode None");
		assert_eq!(dec_none, d_none);
		assert_eq!(d_none.as_ref(), &[] as &[u8]);

		// Raw variant with various lengths
		for &n in &[0usize, 1, 5, 32, 64, 128] {
			let vec: Vec<u8> = vec![0xAB; n];
			let bounded: BoundedVec<u8, ConstU32<128>> = vec.clone().try_into().unwrap();
			let d_raw = TestData::Raw(bounded.clone());
			let enc = d_raw.encode();
			let dec = TestData::decode(&mut &enc[..]).expect("Decode Raw");
			assert_eq!(dec, d_raw);
			assert_eq!(d_raw.as_ref(), vec.as_slice());
		}

		// Digest variant
		let hash = [0x11u8; 32];
		let d_digest = TestData::Digest(hash);
		let enc_digest = d_digest.encode();
		let dec_digest = TestData::decode(&mut &enc_digest[..]).expect("Decode Digest");
		assert_eq!(dec_digest, d_digest);
		assert_eq!(d_digest.as_ref(), &hash[..]);

		// CID variant
		let cid = [0x22u8; 64];
		let d_cid = TestData::CID(cid);
		let enc_cid = d_cid.encode();
		let dec_cid = TestData::decode(&mut &enc_cid[..]).expect("Decode CID");
		assert_eq!(dec_cid, d_cid);
		assert_eq!(d_cid.as_ref(), &cid[..]);
	}
}
