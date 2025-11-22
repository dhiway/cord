use crate::client::submit::TxOutcome;
use crate::client::OriginClient;
use crate::extrinsic::builder::DynamicCallBuilder;
use crate::types::error::OriginSdkError;
use crate::types::{EntityInfoView, EntityOverview, PacketStateView, RegistryStateView};
use origin_primitives::{PacketPointer, Ss58Identifier};
use scale_value::Value;

pub struct Domain<'a> {
	client: &'a OriginClient,
}

impl<'a> Domain<'a> {
	pub fn new(client: &'a OriginClient) -> Self {
		Self { client }
	}

	pub fn entity(&self) -> EntityClient<'a> {
		EntityClient { client: self.client }
	}

	pub fn registry(&self) -> RegistryClient<'a> {
		RegistryClient { client: self.client }
	}

	pub fn packet(&self) -> PacketClient<'a> {
		PacketClient { client: self.client }
	}

	pub fn token(&self) -> TokenClient<'a> {
		TokenClient { client: self.client }
	}
}

pub struct EntityClient<'a> {
	client: &'a OriginClient,
}

impl<'a> EntityClient<'a> {
	pub async fn overview(&self, entity: Ss58Identifier) -> Result<EntityOverview, OriginSdkError> {
		self.client.entity().overview(entity).await.map(EntityOverview::from)
	}

	pub async fn details(&self, entity: Ss58Identifier) -> Result<EntityInfoView, OriginSdkError> {
		self.client.entity().details(entity).await
	}

	pub fn tx(&self) -> EntityTx<'a> {
		EntityTx { client: self.client }
	}

	pub async fn nym(&self, entity: Ss58Identifier) -> Result<Option<Vec<u8>>, OriginSdkError> {
		self.client.entity().nym(entity).await
	}
}

pub struct EntityTx<'a> {
	client: &'a OriginClient,
}

impl<'a> EntityTx<'a> {
	pub fn set_info(&self, info: Value) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new().call("Entity", "set_info", vec![info])
	}

	pub async fn submit_set_info(&self, info: Value) -> Result<TxOutcome, OriginSdkError> {
		let call = self.set_info(info);
		self.client
			.tx()
			.submit(&call.pallet, &call.function, call.args)
			.await?
			.wait_finalized()
			.await
	}

	pub fn set_entity_nym(&self, prefix: &str) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new()
			.call("Entity", "set_entity_nym", vec![Value::from_bytes(prefix.as_bytes())])
	}

	pub async fn submit_set_entity_nym(&self, prefix: &str) -> Result<TxOutcome, OriginSdkError> {
		let call = self.set_entity_nym(prefix);
		self.client
			.tx()
			.submit(&call.pallet, &call.function, call.args)
			.await?
			.wait_finalized()
			.await
	}
}

pub struct RegistryClient<'a> {
	client: &'a OriginClient,
}

impl<'a> RegistryClient<'a> {
	pub async fn details(
		&self,
		registry: Ss58Identifier,
	) -> Result<RegistryStateView, OriginSdkError> {
		self.client.registry().details(registry).await
	}

	pub fn tx(&self) -> RegistryTx<'a> {
		RegistryTx { client: self.client }
	}
}

pub struct RegistryTx<'a> {
	client: &'a OriginClient,
}

impl<'a> RegistryTx<'a> {
	pub fn create(
		&self,
		registry_id: &[u8],
		info: &[u8],
	) -> crate::extrinsic::builder::DynamicCall {
		DynamicCallBuilder::new()
			.call("Register", "create", vec![Value::from_bytes(registry_id), Value::from_bytes(info)])
	}

	pub async fn submit_create(
		&self,
		registry_id: &[u8],
		info: &[u8],
	) -> Result<TxOutcome, OriginSdkError> {
		let call = self.create(registry_id, info);
		self.client
			.tx()
			.submit(&call.pallet, &call.function, call.args)
			.await?
			.wait_finalized()
			.await
	}
}

pub struct PacketClient<'a> {
	client: &'a OriginClient,
}

impl<'a> PacketClient<'a> {
	pub async fn state(
		&self,
		packet: PacketPointer,
		version: Option<u32>,
	) -> Result<PacketStateView, OriginSdkError> {
		self.client.packet().state(packet, version).await
	}
}

pub struct TokenClient<'a> {
	client: &'a OriginClient,
}

impl<'a> TokenClient<'a> {
	pub async fn timeline(
		&self,
		token: Ss58Identifier,
		start: Option<u32>,
		limit: Option<u32>,
	) -> Result<crate::types::TokenTimelineView, OriginSdkError> {
		self.client.token().timeline(token, start, limit).await
	}

	pub async fn resolve_identifier(
		&self,
		token: Ss58Identifier,
	) -> Result<crate::types::TokenLookupView, OriginSdkError> {
		self.client.token().resolve_identifier(token).await
	}
}
