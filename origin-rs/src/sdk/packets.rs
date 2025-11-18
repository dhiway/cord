use origin_primitives::view_api::AuthorizationRequest;

use crate::{
	client::Client,
	sdk::{
		error::{OriginError, Result},
		types::{PacketId, PacketState, RegisterId},
		wire,
	},
};

/// Public-facing Packet API.
pub struct PacketApi<'a> {
	client: &'a Client,
}

impl<'a> PacketApi<'a> {
	pub(crate) fn new(client: &'a Client) -> Self {
		Self { client }
	}

	/// Fetch a packet snapshot by registry + packet id.
	pub async fn state(
		&self,
		auth: &AuthorizationRequest,
		registry: &RegisterId,
		packet: &PacketId,
		version: Option<u32>,
	) -> Result<PacketState> {
		wire::packet::fetch_packet_state(self.client, auth, registry, packet, version).await
	}

	/// Fetch a packet snapshot by token identifier.
	pub async fn state_by_token(
		&self,
		auth: &AuthorizationRequest,
		token: &PacketId,
		version: Option<u32>,
	) -> Result<Option<PacketState>> {
		wire::packet::fetch_packet_by_token(self.client, auth, token, version).await
	}

	/// Placeholder for sending packet updates.
	pub async fn send(
		&self,
		_signer: &impl subxt::tx::Signer<crate::params::config::OriginConfig>,
		_packet: &PacketState,
	) -> Result<()> {
		Err(OriginError::Unsupported("packet send not implemented yet"))
	}
}
