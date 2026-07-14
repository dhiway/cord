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

pub mod entity;
pub mod packet;
pub mod registry;
pub mod token;

use crate::client::{signer::OriginSigner, OriginClient};

/// Unified query facade regrouped by pallet.
pub struct Query<'a> {
	client: &'a OriginClient,
}

impl<'a> Query<'a> {
	pub fn new(client: &'a OriginClient) -> Self {
		Self { client }
	}

	pub fn using(&self, signer: OriginSigner) -> QueryWithSigner<'a> {
		QueryWithSigner { client: self.client, signer }
	}
}

/// Query facade with an attached signer for view authorization and tx shortcuts.
pub struct QueryWithSigner<'a> {
	client: &'a OriginClient,
	signer: OriginSigner,
}

impl<'a> QueryWithSigner<'a> {
	pub fn entity(&self) -> entity::EntityClientWithSigner<'a> {
		entity::EntityClientWithSigner::new(self.client, self.signer.clone())
	}

	pub fn registry(&self) -> registry::RegistryClientWithSigner<'a> {
		registry::RegistryClientWithSigner::new(self.client, self.signer.clone())
	}

	pub fn packet(&self) -> packet::PacketClientWithSigner<'a> {
		packet::PacketClientWithSigner::new(self.client, self.signer.clone())
	}

	pub fn token(&self) -> token::TokenClientWithSigner<'a> {
		token::TokenClientWithSigner::new(self.client, self.signer.clone())
	}
}
