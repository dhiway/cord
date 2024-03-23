// This file is part of CORD – https://cord.network

// Copyright (C) Parity Technologies (UK) Ltd.
// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later
// Adapted to meet the requirements of the CORD project.

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

//! Identity pallet types.

use super::*;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	traits::{ConstU32, Get},
	BoundedVec, CloneNoBound, PartialEqNoBound, RuntimeDebugNoBound,
};
use scale_info::{
	build::{Fields, Variants},
	Path, Type, TypeInfo,
};
use sp_runtime::{traits::Member, RuntimeDebug};
use sp_std::{fmt::Debug, iter::once, prelude::*};

/// An identifier for a single name registrar/identity verification service.
pub type RegistrarIndex = u32;

/// Either underlying data blob if it is at most 32 bytes, or a hash of it. If
/// the data is greater than 32-bytes then it will be truncated when encoding.
///
/// Can also be `None`.
#[derive(Clone, Eq, PartialEq, RuntimeDebug, MaxEncodedLen)]
pub enum Data {
	/// No data here.
	None,
	/// The data is stored directly.
	Raw(BoundedVec<u8, ConstU32<32>>),
	/// Only the Blake2 hash of the data is stored. The preimage of the hash may
	/// be retrieved through some hash-lookup service.
	BlakeTwo256([u8; 32]),
	/// Only the SHA2-256 hash of the data is stored. The preimage of the hash
	/// may be retrieved through some hash-lookup service.
	Sha256([u8; 32]),
	/// Only the Keccak-256 hash of the data is stored. The preimage of the hash
	/// may be retrieved through some hash-lookup service.
	Keccak256([u8; 32]),
	/// Only the SHA3-256 hash of the data is stored. The preimage of the hash
	/// may be retrieved through some hash-lookup service.
	ShaThree256([u8; 32]),
}

impl Data {
	pub fn is_none(&self) -> bool {
		self == &Data::None
	}
}

impl Decode for Data {
	fn decode<I: codec::Input>(input: &mut I) -> sp_std::result::Result<Self, codec::Error> {
		let b = input.read_byte()?;
		Ok(match b {
			0 => Data::None,
			n @ 1..=33 => {
				let mut r: BoundedVec<_, _> = vec![0u8; n as usize - 1]
					.try_into()
					.expect("bound checked in match arm condition; qed");
				input.read(&mut r[..])?;
				Data::Raw(r)
			},
			34 => Data::BlakeTwo256(<[u8; 32]>::decode(input)?),
			35 => Data::Sha256(<[u8; 32]>::decode(input)?),
			36 => Data::Keccak256(<[u8; 32]>::decode(input)?),
			37 => Data::ShaThree256(<[u8; 32]>::decode(input)?),
			_ => return Err(codec::Error::from("invalid leading byte")),
		})
	}
}

impl Encode for Data {
	fn encode(&self) -> Vec<u8> {
		match self {
			Data::None => vec![0u8; 1],
			Data::Raw(ref x) => {
				let l = x.len().min(32);
				let mut r = vec![l as u8 + 1; l + 1];
				r[1..].copy_from_slice(&x[..l]);
				r
			},
			Data::BlakeTwo256(ref h) => once(34u8).chain(h.iter().cloned()).collect(),
			Data::Sha256(ref h) => once(35u8).chain(h.iter().cloned()).collect(),
			Data::Keccak256(ref h) => once(36u8).chain(h.iter().cloned()).collect(),
			Data::ShaThree256(ref h) => once(37u8).chain(h.iter().cloned()).collect(),
		}
	}
}
impl codec::EncodeLike for Data {}

/// Add a Raw variant with the given index and a fixed sized byte array
macro_rules! data_raw_variants {
    ($variants:ident, $(($index:literal, $size:literal)),* ) => {
		$variants
		$(
			.variant(concat!("Raw", stringify!($size)), |v| v
				.index($index)
				.fields(Fields::unnamed().field(|f| f.ty::<[u8; $size]>()))
			)
		)*
    }
}

impl TypeInfo for Data {
	type Identity = Self;

	fn type_info() -> Type {
		let variants = Variants::new().variant("None", |v| v.index(0));

		// create a variant for all sizes of Raw data from 0-32
		let variants = data_raw_variants!(
			variants,
			(1, 0),
			(2, 1),
			(3, 2),
			(4, 3),
			(5, 4),
			(6, 5),
			(7, 6),
			(8, 7),
			(9, 8),
			(10, 9),
			(11, 10),
			(12, 11),
			(13, 12),
			(14, 13),
			(15, 14),
			(16, 15),
			(17, 16),
			(18, 17),
			(19, 18),
			(20, 19),
			(21, 20),
			(22, 21),
			(23, 22),
			(24, 23),
			(25, 24),
			(26, 25),
			(27, 26),
			(28, 27),
			(29, 28),
			(30, 29),
			(31, 30),
			(32, 31),
			(33, 32)
		);

		let variants = variants
			.variant("BlakeTwo256", |v| {
				v.index(34).fields(Fields::unnamed().field(|f| f.ty::<[u8; 32]>()))
			})
			.variant("Sha256", |v| {
				v.index(35).fields(Fields::unnamed().field(|f| f.ty::<[u8; 32]>()))
			})
			.variant("Keccak256", |v| {
				v.index(36).fields(Fields::unnamed().field(|f| f.ty::<[u8; 32]>()))
			})
			.variant("ShaThree256", |v| {
				v.index(37).fields(Fields::unnamed().field(|f| f.ty::<[u8; 32]>()))
			});

		Type::builder().path(Path::new("Data", module_path!())).variant(variants)
	}
}

impl Default for Data {
	fn default() -> Self {
		Self::None
	}
}

/// An attestation of a registrar over how accurate some `IdentityInfo` is in
/// describing an account.
#[derive(Copy, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub enum Judgement {
	/// The default value; no opinion is held.
	Unknown,
	/// No judgement is yet in place, but a request is placed for providing one.
	Requested,
	/// The data appears to be reasonably acceptable in terms of its accuracy,
	/// however no in depth checks (such as in-person meetings or formal KYC)
	/// have been conducted.
	Reasonable,
	/// The target is known directly by the registrar and the registrar can
	/// fully attest to the the data's accuracy.
	KnownGood,
	/// The data was once good but is currently out of date. There is no
	/// malicious intent in the inaccuracy. This judgement can be removed
	/// through updating the data.
	OutOfDate,
	/// The data is imprecise or of sufficiently low-quality to be problematic.
	/// It is not indicative of malicious intent. This judgement can be removed
	/// through updating the data.
	LowQuality,
	/// The data is erroneous. This may be indicative of malicious intent. This
	/// cannot be removed except by the registrar.
	Erroneous,
}

impl Judgement {
	/// Returns `true` if this judgement is indicative of a request being
	/// placed.
	pub(crate) fn has_requested(&self) -> bool {
		matches!(self, Judgement::Requested)
	}

	/// Returns `true` if this judgement is one that should not be generally be
	/// replaced outside of specialized handlers.
	pub(crate) fn is_sticky(&self) -> bool {
		matches!(self, Judgement::Requested | Judgement::Erroneous)
	}
}

/// Information concerning the identity of the controller of an account.
pub trait IdentityInformationProvider:
	Encode + Decode + MaxEncodedLen + Clone + Debug + Eq + PartialEq + TypeInfo + Default
{
	/// Type capable of holding information on which identity fields are set.
	type FieldsIdentifier: Member + Encode + Decode + MaxEncodedLen + TypeInfo + Default;

	/// Check if an identity registered information for some given `fields`.
	fn has_identity(&self, fields: Self::FieldsIdentifier) -> bool;

	/// Create a basic instance of the identity information.
	#[cfg(feature = "runtime-benchmarks")]
	fn create_identity_info() -> Self;

	/// The identity information representation for all identity fields enabled.
	#[cfg(feature = "runtime-benchmarks")]
	fn all_fields() -> Self::FieldsIdentifier;
}

/// Information on an identity along with judgements from registrars.
///
/// NOTE: This is stored separately primarily to facilitate the addition of
/// extra fields in a backwards compatible way through a specialized `Decode`
/// impl.
#[derive(
	CloneNoBound, Encode, Eq, MaxEncodedLen, PartialEqNoBound, RuntimeDebugNoBound, TypeInfo,
)]
#[codec(mel_bound())]
#[scale_info(skip_type_params(MaxJudgements))]
pub struct Registration<
	AccountId: Encode + Decode + Clone + Debug + Eq + PartialEq + MaxEncodedLen,
	MaxJudgements: Get<u32>,
	IdentityInfo: IdentityInformationProvider,
> {
	/// Judgements from the registrars on this identity. Stored ordered by
	/// `RegistrarIndex`. There may be only a single judgement from each
	/// registrar.
	pub judgements: BoundedVec<(AccountId, Judgement), MaxJudgements>,

	/// Information on the identity.
	pub info: IdentityInfo,
}

impl<
		AccountId: Encode + Decode + Clone + Debug + Eq + PartialEq + MaxEncodedLen,
		MaxJudgements: Get<u32>,
		IdentityInfo: IdentityInformationProvider,
	> Decode for Registration<AccountId, MaxJudgements, IdentityInfo>
{
	fn decode<I: codec::Input>(input: &mut I) -> sp_std::result::Result<Self, codec::Error> {
		let (judgements, info) = Decode::decode(&mut AppendZerosInput::new(input))?;
		Ok(Self { judgements, info })
	}
}

/// Information concerning a registrar.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct RegistrarInfo<
	AccountId: Encode + Decode + Clone + Debug + Eq + PartialEq,
	IdField: Encode + Decode + Clone + Debug + Default + Eq + PartialEq + TypeInfo + MaxEncodedLen,
> {
	/// The account of the registrar.
	pub account: AccountId,

	/// Relevant fields for this registrar. Registrar judgements are limited to attestations on
	/// these fields.
	pub fields: IdField,
}

/// Authority properties for a given pallet configuration.
pub type AuthorityPropertiesOf<T> = AuthorityProperties<Suffix<T>>;

/// The number of usernames that an authority may allocate.
type Allocation = u32;
/// A byte vec used to represent a username.
pub(crate) type Suffix<T> = BoundedVec<u8, <T as Config>::MaxSuffixLength>;

/// Properties of a username authority.
#[derive(Clone, Encode, Decode, MaxEncodedLen, TypeInfo, PartialEq, Debug)]
pub struct AuthorityProperties<Suffix> {
	/// The suffix added to usernames granted by this authority. Will be appended to usernames; for
	/// example, a suffix of `wallet` will result in `.wallet` being appended to a user's selected
	/// name.
	pub suffix: Suffix,
	/// The number of usernames remaining that this authority can grant.
	pub allocation: Allocation,
}

/// A byte vec used to represent a username.
pub(crate) type Username<T> = BoundedVec<u8, <T as Config>::MaxUsernameLength>;

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn manual_data_type_info() {
		let mut registry = scale_info::Registry::new();
		let type_id = registry.register_type(&scale_info::meta_type::<Data>());
		let registry: scale_info::PortableRegistry = registry.into();
		let type_info = registry.resolve(type_id.id).unwrap();

		let check_type_info = |data: &Data| {
			let variant_name = match data {
				Data::None => "None".to_string(),
				Data::BlakeTwo256(_) => "BlakeTwo256".to_string(),
				Data::Sha256(_) => "Sha256".to_string(),
				Data::Keccak256(_) => "Keccak256".to_string(),
				Data::ShaThree256(_) => "ShaThree256".to_string(),
				Data::Raw(bytes) => format!("Raw{}", bytes.len()),
			};
			if let scale_info::TypeDef::Variant(variant) = &type_info.type_def {
				let variant = variant
					.variants
					.iter()
					.find(|v| v.name == variant_name)
					.expect(&format!("Expected to find variant {}", variant_name));

				let field_arr_len = variant
					.fields
					.first()
					.and_then(|f| registry.resolve(f.ty.id))
					.map(|ty| {
						if let scale_info::TypeDef::Array(arr) = &ty.type_def {
							arr.len
						} else {
							panic!("Should be an array type")
						}
					})
					.unwrap_or(0);

				let encoded = data.encode();
				assert_eq!(encoded[0], variant.index);
				assert_eq!(encoded.len() as u32 - 1, field_arr_len);
			} else {
				panic!("Should be a variant type")
			};
		};

		let mut data = vec![
			Data::None,
			Data::BlakeTwo256(Default::default()),
			Data::Sha256(Default::default()),
			Data::Keccak256(Default::default()),
			Data::ShaThree256(Default::default()),
		];

		// A Raw instance for all possible sizes of the Raw data
		for n in 0..32 {
			data.push(Data::Raw(vec![0u8; n as usize].try_into().unwrap()))
		}

		for d in data.iter() {
			check_type_info(d);
		}
	}
}
