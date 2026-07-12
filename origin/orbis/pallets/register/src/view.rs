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

//! View adapters for registry + packet structures.

use crate::*;

use crate::register::LookupSpec;
use alloc::{vec, vec::Vec};
use codec::Encode;
use origin_primitives::{
	element::ElementView,
	identifier::Ss58Identifier,
	packet::{PacketAttributeView, PacketMetadataView, PacketSnapshot, PacketStateView},
	registry::{RegistryAttributeView, RegistryStateView},
};

/// Convert RegistryInfoOf<T> → RegistryStateView (fully flattened)
pub fn build_registry_state_view<T: Config>(
	registry: &Ss58Identifier,
	info: &RegistryInfoOf<T>,
) -> RegistryStateView {
	let attributes = info
		.attributes
		.iter()
		.map(|spec| RegistryAttributeView {
			key: spec.key.to_vec(),
			kind: spec.kind,
			optional: spec.flags.is_optional(),
		})
		.collect::<Vec<_>>();

	let token_spec = match &info.token_spec {
		LookupSpec::Single(k) => vec![k.to_vec()],
		LookupSpec::Combo(list) => list.iter().map(|a| a.to_vec()).collect(),
	};

	let lookup_specs = info
		.lookup_specs
		.iter()
		.map(|spec| match spec {
			LookupSpec::Single(k) => vec![k.to_vec()],
			LookupSpec::Combo(list) => list.iter().map(|a| a.to_vec()).collect(),
		})
		.collect::<Vec<_>>();

	RegistryStateView {
		registry: registry.clone(),
		maintainer: info.maintainer.clone(),
		info: ElementView::from(&info.info),
		kind: info.kind.clone(),
		status: info.status.clone(),
		attributes,
		token_spec,
		lookup_specs,
	}
}

/// Convert a runtime-level PacketSnapshot into an API-facing PacketStateView.
pub fn build_packet_state_view<T: Config>(
	packet: &Ss58Identifier,
	snapshot: &PacketSnapshot<
		T::MaxRawDataLength,
		T::MaxAdditionalAttributes,
		<T as frame_system::Config>::Hash,
	>,
) -> PacketStateView {
	PacketStateView {
		registry: snapshot.state.registry.clone(),
		packet: packet.clone(),
		controller: snapshot.state.controller.clone(),
		status: snapshot.state.status.clone(),
		version: snapshot.state.version,
		registry_status: snapshot.registry_status,
		digest: snapshot.state.digest.encode(),
		attributes: snapshot
			.state
			.attributes
			.iter()
			.map(|(k, v)| PacketAttributeView { key: k.to_vec(), value: ElementView::from(v) })
			.collect(),
	}
}

/// Convert PacketMetadata<Hash> → PacketMetadataView
pub fn build_packet_metadata_view<T: Config>(metadata: &PacketMetadataOf<T>) -> PacketMetadataView {
	PacketMetadataView {
		registry: metadata.registry.clone(),
		controller: metadata.controller.clone(),
		status: metadata.status.clone(),
		latest_version: metadata.latest_version,
		digest: metadata.digest.encode(),
	}
}
