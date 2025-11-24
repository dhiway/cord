use crate::{
	client::{signer::Signer, submit::TxOutcome, OriginClient},
	extrinsic::{
		builder::DynamicCallBuilder,
		calls::packet as packet_calls,
	},
	types::{error::OriginSdkError, PacketStateView},
};
use origin_primitives::PacketPointer;
use scale_value::Value;
use serde_json::Value as JsonValue;

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

	pub fn tx(&self) -> PacketTx<'a> {
		PacketTx { client: self.client }
	}
}

pub struct PacketTx<'a> {
	client: &'a OriginClient,
}

impl<'a> PacketTx<'a> {
	pub fn issue(
		&self,
		registry: impl AsRef<[u8]>,
		body: impl AsRef<[u8]>,
	) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new().call(
			"Register",
			"create_packet",
			vec![Value::from_bytes(registry.as_ref()), Value::from_bytes(body.as_ref())],
		)
	}

	pub async fn submit_issue(
		&self,
		registry: impl AsRef<[u8]>,
		body: impl AsRef<[u8]>,
	) -> Result<TxOutcome, OriginSdkError> {
		let call = self.issue(registry, body);
		self.client
			.tx()?
			.submit(&call.pallet, &call.function, call.args)
			.await?
			.wait_in_block()
			.await
	}

	pub fn update(
		&self,
		registry: impl AsRef<[u8]>,
		packet: impl AsRef<[u8]>,
		attributes: Value,
	) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new().call(
			"Register",
			"update_packet",
			vec![
				Value::from_bytes(registry.as_ref()),
				Value::from_bytes(packet.as_ref()),
				attributes,
			],
		)
	}

	pub fn revoke(
		&self,
		registry: impl AsRef<[u8]>,
		packet: impl AsRef<[u8]>,
	) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new().call(
			"Register",
			"revoke_packet",
			vec![Value::from_bytes(registry.as_ref()), Value::from_bytes(packet.as_ref())],
		)
	}

	pub fn restore(
		&self,
		registry: impl AsRef<[u8]>,
		packet: impl AsRef<[u8]>,
	) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new().call(
			"Register",
			"restore_packet",
			vec![Value::from_bytes(registry.as_ref()), Value::from_bytes(packet.as_ref())],
		)
	}

	pub fn remove(
		&self,
		registry: impl AsRef<[u8]>,
		packet: impl AsRef<[u8]>,
	) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new().call(
			"Register",
			"remove_packet",
			vec![Value::from_bytes(registry.as_ref()), Value::from_bytes(packet.as_ref())],
		)
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

	pub fn tx(&self) -> PacketTxWithSigner<'a, S> {
		PacketTxWithSigner { client: self.client, signer: self.signer.clone() }
	}
}

pub struct PacketTxWithSigner<'a, S: Signer + Clone + 'static> {
	client: &'a OriginClient,
	signer: S,
}

impl<'a, S: Signer + Clone + 'static> PacketTxWithSigner<'a, S> {
	pub fn issue(
		&self,
		registry: impl AsRef<[u8]>,
		body: impl AsRef<[u8]>,
	) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new().call(
			"Register",
			"create_packet",
			vec![Value::from_bytes(registry.as_ref()), Value::from_bytes(body.as_ref())],
		)
	}

	pub async fn submit_issue(
		&self,
		registry: impl AsRef<[u8]>,
		body: impl AsRef<[u8]>,
	) -> Result<TxOutcome, OriginSdkError> {
		let call = self.issue(registry, body);
		self.client
			.tx_with(self.signer.clone())
			.submit(&call.pallet, &call.function, call.args)
			.await?
			.wait_in_block()
			.await
	}

	/// Issue a packet from raw JSON, validating against registry schema via view.
	pub async fn issue_from_raw(
		&self,
		registry: origin_primitives::Ss58Identifier,
		body: JsonValue,
	) -> Result<TxOutcome, OriginSdkError> {
		let attr_triples = self
			.client
			.view_with(self.signer.clone())
			.registry()
			.attributes(registry.clone())
			.await?;
		let registry_view: Vec<origin_primitives::registry::RegistryAttributeView> = attr_triples
			.into_iter()
			.map(|(key, kind, optional)| origin_primitives::registry::RegistryAttributeView {
				key,
				kind,
				optional,
			})
			.collect();

		let metadata = self.client.metadata();
		let call = packet_calls::issue_call(&metadata, registry, &registry_view, &body)?;
		self.client
			.tx_with(self.signer.clone())
			.submit_payload(call)
			.await?
			.wait_in_block()
			.await
	}
}
