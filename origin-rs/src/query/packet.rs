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

use crate::{
	client::{signer::OriginSigner, OriginClient},
	schema,
	types::{error::OriginSdkError, PacketStateViewSdk},
};
use origin_primitives::{PacketPointer, Ss58Identifier};

type Auth = origin_primitives::Authorization<
	origin_primitives::AccountId,
	Vec<u8>,
	origin_primitives::Signature,
>;

pub struct PacketClientWithSigner<'a> {
	client: &'a OriginClient,
	signer: OriginSigner,
}

impl<'a> PacketClientWithSigner<'a> {
	pub(crate) fn new(client: &'a OriginClient, signer: OriginSigner) -> Self {
		Self { client, signer }
	}

	fn view(&self) -> crate::client::ViewClient {
		self.client.view()
	}

	async fn auth(&self, function: &str) -> Result<Auth, OriginSdkError> {
		self.view().authorization_for(&self.signer, "Register", function).await
	}

	/// Packet snapshot by token (optionally at a specific version).
	pub async fn state(
		&self,
		pointer: PacketPointer,
		version: Option<u32>,
	) -> Result<Option<PacketStateViewSdk>, OriginSdkError> {
		let auth = self.auth("packet_state").await?;
		let version_arg = version.or(Some(pointer.version));
		self.view()
			.call("Register", "packet_state", (auth, pointer.registry, pointer.packet, version_arg))
			.await
	}

	/// Resolve a packet snapshot via lookup digest for a registry.
	pub async fn lookup(
		&self,
		registry: Ss58Identifier,
		digest: Vec<u8>,
		version: Option<u32>,
	) -> Result<Option<PacketStateViewSdk>, OriginSdkError> {
		let auth = self.auth("packet_lookup_snapshot").await?;
		self.view()
			.call("Register", "packet_lookup_snapshot", (auth, registry, digest, version))
			.await
	}

	pub async fn state_nested(
		&self,
		pointer: PacketPointer,
		version: Option<u32>,
	) -> Result<Option<schema::packet::PacketNestedValue>, OriginSdkError> {
		let flat = self.state(pointer, version).await?;
		Ok(flat.map(|f| schema::packet::expand_packet_view(&f)))
	}
}
