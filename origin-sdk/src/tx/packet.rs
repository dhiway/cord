use crate::{
	client::{signer::OriginSigner, submit::TxHandle, OriginClient},
	extrinsic::{builder::DynamicCallBuilder, calls::packet as packet_calls},
	schema,
	types::error::OriginSdkError,
};
use codec::Encode;
use scale_value::Value;
use serde_json::Value as JsonValue;

pub struct PacketTx<'a> {
	client: &'a OriginClient,
	signer: OriginSigner,
}

impl<'a> PacketTx<'a> {
	pub(crate) fn new(client: &'a OriginClient, signer: OriginSigner) -> Self {
		Self { client, signer }
	}

	/// Issue a packet from nested attributes, validated against the registry schema view.
	pub async fn submit_issue_from_nested(
		&self,
		registry: origin_primitives::Ss58Identifier,
		nested: &schema::packet::PacketNestedValue,
	) -> Result<TxHandle, OriginSdkError> {
		let schema_view = self
			.client
			.query()
			.using(self.signer.clone())
			.registry()
			.attributes(registry.clone())
			.await?
			.ok_or_else(|| OriginSdkError::View("registry schema not found".into()))?;
		let schema_tuples: Vec<(Vec<u8>, origin_primitives::element::ElementType, bool)> =
			schema_view.iter().map(|s| (s.key.clone(), s.kind, s.optional)).collect();
		let flat = crate::schema::packet::flatten_packet(nested)?;
		validate_packet_against_schema(&flat, &schema_tuples)?;
		let attr_bytes: Vec<(Vec<u8>, Vec<u8>)> =
			flat.into_iter().map(|(k, v)| (k, v.encode())).collect();
		let payload =
			packet_calls::issue_from_flat(&self.client.metadata(), registry, &attr_bytes)?;
		self.client.submit_with(self.signer.clone()).submit_payload(payload).await
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
	) -> Result<TxHandle, OriginSdkError> {
		let call = self.issue(registry, body);
		self.client
			.submit_with(self.signer.clone())
			.submit(&call.pallet, &call.function, call.args)
			.await
	}

	pub fn update_packet(
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

	pub fn revoke_packet(
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

	pub async fn submit_revoke_packet(
		&self,
		registry: origin_primitives::Ss58Identifier,
		packet: origin_primitives::Ss58Identifier,
	) -> Result<TxHandle, OriginSdkError> {
		let payload = packet_calls::revoke_call(&self.client.metadata(), registry, packet)?;
		self.client.submit_with(self.signer.clone()).submit_payload(payload).await
	}

	pub fn restore_packet(
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

	pub async fn submit_restore_packet(
		&self,
		registry: origin_primitives::Ss58Identifier,
		packet: origin_primitives::Ss58Identifier,
	) -> Result<TxHandle, OriginSdkError> {
		let payload = packet_calls::restore_call(&self.client.metadata(), registry, packet)?;
		self.client.submit_with(self.signer.clone()).submit_payload(payload).await
	}

	pub fn delete_packet(
		&self,
		registry: impl AsRef<[u8]>,
		packet: impl AsRef<[u8]>,
	) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new().call(
			"Register",
			"delete_packet",
			vec![Value::from_bytes(registry.as_ref()), Value::from_bytes(packet.as_ref())],
		)
	}

	pub async fn submit_delete_packet(
		&self,
		registry: origin_primitives::Ss58Identifier,
		packet: origin_primitives::Ss58Identifier,
	) -> Result<TxHandle, OriginSdkError> {
		let payload = packet_calls::delete_call(&self.client.metadata(), registry, packet)?;
		self.client.submit_with(self.signer.clone()).submit_payload(payload).await
	}

	pub fn set_packet_status(
		&self,
		registry: impl AsRef<[u8]>,
		packet: impl AsRef<[u8]>,
		status: origin_primitives::packet::PacketStatus,
	) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new().call(
			"Register",
			"set_packet_status",
			vec![
				Value::from_bytes(registry.as_ref()),
				Value::from_bytes(packet.as_ref()),
				Value::from_bytes(&status.encode()),
			],
		)
	}

	pub async fn submit_set_packet_status(
		&self,
		registry: origin_primitives::Ss58Identifier,
		packet: origin_primitives::Ss58Identifier,
		status: origin_primitives::packet::PacketStatus,
	) -> Result<TxHandle, OriginSdkError> {
		let payload =
			packet_calls::set_status_call(&self.client.metadata(), registry, packet, status)?;
		self.client.submit_with(self.signer.clone()).submit_payload(payload).await
	}

	/// Issue a packet from raw JSON, validating against registry schema via view.
	pub async fn issue_from_raw(
		&self,
		registry: origin_primitives::Ss58Identifier,
		body: JsonValue,
	) -> Result<TxHandle, OriginSdkError> {
		let attr_triples = self
			.client
			.query()
			.using(self.signer.clone())
			.registry()
			.attributes(registry.clone())
			.await?
			.ok_or_else(|| OriginSdkError::View("registry schema not found".into()))?;
		let registry_view: Vec<origin_primitives::registry::RegistryAttributeView> = attr_triples;

		let metadata = self.client.metadata();
		let call = packet_calls::issue_call(&metadata, registry, &registry_view, &body)?;
		self.client.submit_with(self.signer.clone()).submit_payload(call).await
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
		_ => Err(OriginSdkError::Schema("element type mismatch".into())),
	}
}
