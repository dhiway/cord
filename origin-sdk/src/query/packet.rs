use crate::{
	client::{signer::Signer, OriginClient, ViewClient},
	schema,
	types::{error::OriginSdkError, PacketStateView},
};
use origin_primitives::PacketPointer;

pub struct PacketClientWithSigner<'a, S: Signer + Clone + 'static> {
	client: &'a OriginClient,
	signer: S,
}

impl<'a, S: Signer + Clone + 'static> PacketClientWithSigner<'a, S> {
	pub(crate) fn new(client: &'a OriginClient, signer: S) -> Self {
		Self { client, signer }
	}

	fn view(&self) -> ViewClient {
		self.client.view_with(self.signer.clone())
	}

	/// Packet snapshot by token (optionally at a specific version).
	pub async fn state(
		&self,
		packet: PacketPointer,
		version: Option<u32>,
	) -> Result<PacketStateView, OriginSdkError> {
		self.view().packet().state(packet, version).await
	}

	/// Resolve a packet snapshot via lookup digest for a registry.
	pub async fn lookup(
		&self,
		registry: origin_primitives::Ss58Identifier,
		digest: Vec<u8>,
		version: Option<u32>,
	) -> Result<PacketStateView, OriginSdkError> {
		self.view().registry().lookup_snapshot(registry, digest, version).await
	}

	pub async fn state_nested(
		&self,
		packet: PacketPointer,
		version: Option<u32>,
	) -> Result<schema::packet::PacketNestedValue, OriginSdkError> {
		let flat = self.state(packet, version).await?;
		Ok(schema::packet::expand_packet_view(&flat))
	}
}
