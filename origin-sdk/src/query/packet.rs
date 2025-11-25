use crate::{
	client::{signer::Signer, OriginClient},
	schema,
	types::{error::OriginSdkError, PacketStateView},
};
use origin_primitives::PacketPointer;

pub struct PacketClient<'a> {
	client: &'a OriginClient,
}

impl<'a> PacketClient<'a> {
	pub(crate) fn new(client: &'a OriginClient) -> Self {
		Self { client }
	}

	pub fn using<S>(&self, signer: S) -> PacketClientWithSigner<'a, S>
	where
		S: Signer + Clone + 'static,
	{
		PacketClientWithSigner { client: self.client, signer }
	}

	pub async fn state(
		&self,
		packet: PacketPointer,
		version: Option<u32>,
	) -> Result<PacketStateView, OriginSdkError> {
		self.client.view()?.packet().state(packet, version).await
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

pub struct PacketClientWithSigner<'a, S: Signer + Clone + 'static> {
	client: &'a OriginClient,
	signer: S,
}

impl<'a, S: Signer + Clone + 'static> PacketClientWithSigner<'a, S> {
	pub(crate) fn new(client: &'a OriginClient, signer: S) -> Self {
		Self { client, signer }
	}

	pub async fn state(
		&self,
		packet: PacketPointer,
		version: Option<u32>,
	) -> Result<PacketStateView, OriginSdkError> {
		self.client.view_with(self.signer.clone()).packet().state(packet, version).await
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
