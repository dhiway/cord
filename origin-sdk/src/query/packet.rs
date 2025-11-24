use crate::{
	client::{signer::Signer, submit::TxOutcome, OriginClient},
	extrinsic::{builder::DynamicCallBuilder, calls::packet as packet_calls},
	schema,
	types::{error::OriginSdkError, PacketStateView},
};
use codec::Encode;
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

	pub async fn state_nested(
		&self,
		packet: PacketPointer,
		version: Option<u32>,
	) -> Result<schema::packet::PacketNestedValue, OriginSdkError> {
		let flat = self.state(packet, version).await?;
		Ok(schema::packet::expand_packet_view(&flat))
	}

	pub fn tx(&self) -> PacketTx<'a> {
		PacketTx { client: self.client }
	}
}

pub struct PacketTx<'a> {
	client: &'a OriginClient,
}

impl<'a> PacketTx<'a> {
	/// Issue a packet from nested attributes, validated against the registry schema view.
	pub async fn submit_issue_from_nested(
		&self,
		registry: origin_primitives::Ss58Identifier,
		nested: &schema::packet::PacketNestedValue,
	) -> Result<TxOutcome, OriginSdkError> {
		let schema_view = self.client.view()?.registry().attributes(registry.clone()).await?;
		let flat = crate::schema::packet::flatten_packet(nested)?;
		validate_packet_against_schema(&flat, &schema_view)?;
		let attr_bytes: Vec<(Vec<u8>, Vec<u8>)> =
			flat.into_iter().map(|(k, v)| (k, v.encode())).collect();
		let payload =
			packet_calls::issue_from_flat(&self.client.metadata(), registry, &attr_bytes)?;
		self.client.tx()?.submit_payload(payload).await?.wait_in_block().await
	}

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

	pub async fn state_nested(
		&self,
		packet: PacketPointer,
		version: Option<u32>,
	) -> Result<schema::packet::PacketNestedValue, OriginSdkError> {
		let flat = self.state(packet, version).await?;
		Ok(schema::packet::expand_packet_view(&flat))
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
	/// Issue a packet from nested attributes, validated against the registry schema view.
	pub async fn submit_issue_from_nested(
		&self,
		registry: origin_primitives::Ss58Identifier,
		nested: &schema::packet::PacketNestedValue,
	) -> Result<TxOutcome, OriginSdkError> {
		let schema_view = self
			.client
			.view_with(self.signer.clone())
			.registry()
			.attributes(registry.clone())
			.await?;
		let flat = crate::schema::packet::flatten_packet(nested)?;
		validate_packet_against_schema(&flat, &schema_view)?;
		let attr_bytes: Vec<(Vec<u8>, Vec<u8>)> =
			flat.into_iter().map(|(k, v)| (k, v.encode())).collect();
		let payload =
			packet_calls::issue_from_flat(&self.client.metadata(), registry, &attr_bytes)?;
		self.client
			.tx_with(self.signer.clone())
			.submit_payload(payload)
			.await?
			.wait_in_block()
			.await
	}

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

pub fn validate_packet_against_schema(
	body: &[(Vec<u8>, origin_primitives::element::ElementView)],
	schema: &[(Vec<u8>, origin_primitives::element::ElementType, bool)],
) -> Result<(), OriginSdkError> {
	for (key, kind, optional) in schema.iter() {
		let found = body.iter().find(|(k, _)| k == key);
		match (found, optional) {
			(Some((_, val)), _) => validate_element_type(kind, val)?,
			(None, true) => {},
			(None, false) => {
				return Err(OriginSdkError::Schema(format!(
					"missing required attribute '{}'",
					String::from_utf8_lossy(key)
				)))
			},
		}
	}
	for (k, _) in body.iter() {
		if !schema.iter().any(|(sk, _, _)| sk == k) {
			return Err(OriginSdkError::Schema(format!(
				"attribute '{}' not defined in registry schema",
				String::from_utf8_lossy(k)
			)));
		}
	}
	Ok(())
}

fn validate_element_type(
	expected: &origin_primitives::element::ElementType,
	val: &origin_primitives::element::ElementView,
) -> Result<(), OriginSdkError> {
	match (expected, val) {
		(
			origin_primitives::element::ElementType::Bool,
			origin_primitives::element::ElementView::Bool(_),
		) => Ok(()),
		(
			origin_primitives::element::ElementType::U64,
			origin_primitives::element::ElementView::U64(_),
		) => Ok(()),
		(
			origin_primitives::element::ElementType::U128,
			origin_primitives::element::ElementView::U128(_),
		) => Ok(()),
		(
			origin_primitives::element::ElementType::Hash,
			origin_primitives::element::ElementView::Hash(_),
		) => Ok(()),
		(
			origin_primitives::element::ElementType::Token,
			origin_primitives::element::ElementView::Token(_),
		) => Ok(()),
		(
			origin_primitives::element::ElementType::Cid,
			origin_primitives::element::ElementView::Cid(_),
		) => Ok(()),
		(
			origin_primitives::element::ElementType::Raw,
			origin_primitives::element::ElementView::Raw(_),
		) => Ok(()),
		(
			origin_primitives::element::ElementType::None,
			origin_primitives::element::ElementView::None,
		) => Ok(()),
		(_, origin_primitives::element::ElementView::Raw(_)) => Ok(()), // lenient fallback
		(other, got) => Err(OriginSdkError::Schema(format!(
			"type mismatch: expected {:?}, got {:?}",
			other, got
		))),
	}
}
