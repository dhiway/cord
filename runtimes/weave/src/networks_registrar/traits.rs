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

use super::primitives::Id as NetworkId;
use alloc::vec::Vec;

/// Network registration API.
pub trait Registrar {
	///The account ID type that encodes a parachain manager ID.
	type AccountId;

	/// Report the manager (permissioned owner) of a parachain, if there is one.
	#[allow(dead_code)]
	fn manager_of(id: NetworkId) -> Option<Self::AccountId>;

	/// All lease holding parachains. Ordered ascending by `ParaId`. On-demand
	/// parachains are not included.
	fn networks() -> Vec<NetworkId>;

	/// Return if a `ParaId` is a lease holding Parachain.
	fn is_network(id: NetworkId) -> bool {
		Self::networks().binary_search(&id).is_ok()
	}

	/// Return if a `ParaId` is registered in the system.
	#[allow(dead_code)]
	fn is_registered(id: NetworkId) -> bool {
		Self::is_network(id)
	}
}
