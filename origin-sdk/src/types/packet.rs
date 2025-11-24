pub use origin_primitives::{PacketMetadataView, PacketPointer, PacketStateView, PacketStatus};

use codec::{Decode, Encode};
use frame_support::traits::ConstU32;
use origin_primitives::{
	attribute::{Attributes, Element},
	packet::{PacketAttributeView, PacketSnapshot, PacketState},
	Ss58Identifier,
};
use sp_core::H256;

#[derive(Clone, Debug, Decode)]
pub struct PacketTimelineView {
	pub events: Vec<String>,
}

pub type MaxRawDataLength = ConstU32<4096>;
pub type MaxAdditionalAttributes = ConstU32<32>;

pub type PacketElement = Element<MaxRawDataLength>;
pub type PacketAttributes = Attributes<MaxRawDataLength, MaxAdditionalAttributes>;
pub type PacketStateInternal = PacketState<MaxRawDataLength, MaxAdditionalAttributes, H256>;
pub type PacketSnapshotInternal = PacketSnapshot<MaxRawDataLength, MaxAdditionalAttributes, H256>;

pub fn packet_state_view_from_snapshot(
	packet: &Ss58Identifier,
	snapshot: &PacketSnapshotInternal,
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
			.map(|(k, v)| PacketAttributeView { key: k.to_vec(), value: v.into() })
			.collect(),
	}
}
