use crate::{
	client::{submit::TxOutcome, OriginClient},
	extrinsic::builder::DynamicCallBuilder,
	types::{error::OriginSdkError, PacketStateView},
};
use origin_primitives::PacketPointer;
use scale_value::Value;

pub struct PacketClient<'a> {
	client: &'a OriginClient,
}

impl<'a> PacketClient<'a> {
	pub(crate) fn new(client: &'a OriginClient) -> Self {
		Self { client }
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
