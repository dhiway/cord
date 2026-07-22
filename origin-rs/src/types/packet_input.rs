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

use codec::{Decode, Encode};
use frame_support::traits::ConstU32;
use origin_primitives::{
	attribute::{Attribute, Attributes},
	element::Elum,
};
use scale_info::TypeInfo;

/// Bounds mirror runtime constants (see pallet-register).
pub type MaxRawDataLength = ConstU32<4096>;
pub type MaxAdditionalAttributes = ConstU32<32>;

/// Element used for packet attributes in extrinsic inputs.
pub type PacketElementInput = Elum<MaxRawDataLength>;

/// Attribute collection used by packet extrinsics.
pub type PacketAttributesInput = Attributes<MaxRawDataLength, MaxAdditionalAttributes>;

/// Single attribute entry for packets.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub struct PacketAttributeInput {
	pub key: Attribute,
	pub value: PacketElementInput,
}
