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

pub use origin_primitives::{identifier::DecodedIdentifier, Ss58Identifier};
/// Helper to render an identifier as a human SS58/base58 string.
pub fn ss58_to_string(id: &Ss58Identifier) -> String {
	id.to_string_lossy()
}

/// Render an AccountId32 with a given ss58 prefix (default Origin = 29).
pub fn account_to_ss58(account: &subxt::utils::AccountId32, prefix: u16) -> String {
	use sp_core::crypto::{AccountId32 as CoreAccountId32, Ss58AddressFormat, Ss58Codec};
	let fmt = Ss58AddressFormat::custom(prefix);
	let core = CoreAccountId32::from(account.0);
	core.to_ss58check_with_version(fmt)
}
