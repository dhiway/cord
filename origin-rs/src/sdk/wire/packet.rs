use origin_primitives::view_api::{
	AuthorizationRequest, RegisterPacketSnapshotByTokenRequest, RegisterPacketSnapshotRequest,
};

use crate::{
	client::Client,
	query::register::PacketSnapshotView,
	sdk::{
		error::Result,
		types::{PacketId, PacketState, RegisterId},
	},
};

pub(crate) async fn fetch_packet_state(
	client: &Client,
	auth: &AuthorizationRequest,
	registry: &RegisterId,
	packet: &PacketId,
	version: Option<u32>,
) -> Result<PacketState> {
	let req = RegisterPacketSnapshotRequest {
		auth: auth.clone(),
		registry: registry.clone(),
		packet: packet.clone(),
		version,
	};
	let view: PacketSnapshotView = client.query().register().packet_snapshot(&req).await?;
	Ok(PacketState::from_dev(packet.clone(), view.state))
}

pub(crate) async fn fetch_packet_by_token(
	client: &Client,
	auth: &AuthorizationRequest,
	token: &PacketId,
	version: Option<u32>,
) -> Result<Option<PacketState>> {
	let req =
		RegisterPacketSnapshotByTokenRequest { auth: auth.clone(), token: token.clone(), version };
	let view: Option<PacketSnapshotView> =
		client.query().register().packet_snapshot_by_token(&req).await?;
	Ok(view.map(|snapshot| PacketState::from_dev(token.clone(), snapshot.state)))
}
