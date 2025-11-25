use crate::{
	client::{signer::Signer, OriginClient},
	schema,
	types::{error::OriginSdkError, PacketStateViewSdk},
};
use codec::Encode;
use origin_primitives::{PacketPointer, Ss58Identifier};

type Auth = origin_primitives::Authorization<
	origin_primitives::AccountId,
	Vec<u8>,
	origin_primitives::Signature,
>;

pub struct PacketClientWithSigner<'a, S: Signer + Clone + 'static> {
	client: &'a OriginClient,
	signer: S,
}

impl<'a, S: Signer + Clone + 'static> PacketClientWithSigner<'a, S> {
	pub(crate) fn new(client: &'a OriginClient, signer: S) -> Self {
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
		self.view().call(
			"Register",
			"packet_state",
			vec![auth.encode(), pointer.registry.encode(), pointer.packet.encode(), version_arg.encode()],
		).await
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
			.call(
				"Register",
				"packet_lookup_snapshot",
				vec![auth.encode(), registry.encode(), digest.encode(), version.encode()],
			)
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
