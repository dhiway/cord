use origin_primitives::view_api::AuthorizationRequest;

use crate::{
	client::Client,
	sdk::{
		error::Result,
		types::{PacketId, PacketOverview, PacketState, RegisterId},
		wire,
	},
	tx::TxOptions,
};
use origin_primitives::registry::RegistryInfoView;

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

	/// Create a packet using registry schema and attribute JSON.
	pub async fn create(
		&self,
		signer: &impl subxt::tx::Signer<crate::params::config::OriginConfig>,
		registry: &RegisterId,
		attributes_json: serde_json::Value,
		schema: &RegistryInfoView,
		opts: TxOptions,
	) -> Result<()> {
		wire::packet::create_packet(self.client, signer, registry, attributes_json, schema, opts)
			.await
	}

	/// Update an existing packet using registry schema and partial attribute JSON.
	pub async fn update(
		&self,
		signer: &impl subxt::tx::Signer<crate::params::config::OriginConfig>,
		registry: &RegisterId,
		packet: &PacketId,
		attributes_json: serde_json::Value,
		schema: &RegistryInfoView,
		opts: TxOptions,
	) -> Result<()> {
		wire::packet::update_packet(
			self.client,
			signer,
			registry,
			packet,
			attributes_json,
			schema,
			opts,
		)
		.await
	}

	/// Create a packet from typed attributes (full payload).
	pub async fn create_typed(
		&self,
		signer: &impl subxt::tx::Signer<crate::params::config::OriginConfig>,
		registry: &RegisterId,
		attributes: Vec<crate::sdk::types::Attribute>,
		schema: &RegistryInfoView,
		opts: TxOptions,
	) -> Result<()> {
		wire::packet::create_packet_typed(self.client, signer, registry, attributes, schema, opts)
			.await
	}

	/// Update a packet from typed attributes (partial payload).
	pub async fn update_typed(
		&self,
		signer: &impl subxt::tx::Signer<crate::params::config::OriginConfig>,
		registry: &RegisterId,
		packet: &PacketId,
		attributes: Vec<crate::sdk::types::Attribute>,
		schema: &RegistryInfoView,
		opts: TxOptions,
	) -> Result<()> {
		wire::packet::update_packet_typed(
			self.client,
			signer,
			registry,
			packet,
			attributes,
			schema,
			opts,
		)
		.await
	}

	/// Packet overview composed from snapshot + token timeline (limit 20).
	pub async fn overview(
		&self,
		auth: &origin_primitives::view_api::AuthorizationRequest,
		registry: &RegisterId,
		packet: &PacketId,
		version: Option<u32>,
	) -> Result<PacketOverview> {
		let state = self.state(auth, registry, packet, version).await?;
		let metadata = wire::packet::fetch_metadata(self.client, auth, registry, packet).await?;
		let metadata = crate::sdk::types::PacketMetadata::from_view(packet.clone(), metadata);

		let (timeline, _) =
			wire::token::timeline(self.client, auth, packet, None, Some(20)).await?;

		Ok(PacketOverview { metadata, state, timeline })
	}
}
