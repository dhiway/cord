use crate::client::OriginClient;
use crate::types::error::OriginSdkError;
use crate::types::{TokenLookupView, TokenTimelineView};
use origin_primitives::Ss58Identifier;

pub struct TokenClient<'a> {
	client: &'a OriginClient,
}

impl<'a> TokenClient<'a> {
	pub(crate) fn new(client: &'a OriginClient) -> Self {
		Self { client }
	}

	pub async fn timeline(
		&self,
		token: Ss58Identifier,
		start: Option<u32>,
		limit: Option<u32>,
	) -> Result<TokenTimelineView, OriginSdkError> {
		self.client.view()?.token().timeline(token, start, limit).await
	}

	pub async fn resolve_identifier(
		&self,
		token: Ss58Identifier,
	) -> Result<TokenLookupView, OriginSdkError> {
		self.client.view()?.token().resolve_identifier(token).await
	}

	pub async fn pallet_index_of(&self, name: Vec<u8>) -> Result<u16, OriginSdkError> {
		self.client.view()?.token().pallet_index_of(name).await
	}

	pub async fn pallet_name(&self, index: u16) -> Result<Vec<u8>, OriginSdkError> {
		self.client.view()?.token().pallet_name(index).await
	}

	pub async fn next_pallet_index(&self) -> Result<u16, OriginSdkError> {
		self.client.view()?.token().next_pallet_index().await
	}

	pub async fn genesis_network_id(&self) -> Result<u32, OriginSdkError> {
		self.client.view()?.token().genesis_network_id().await
	}

	pub async fn state_version(&self, token: Ss58Identifier) -> Result<u32, OriginSdkError> {
		self.client.view()?.token().state_version(token).await
	}

	pub async fn state_event(
		&self,
		token: Ss58Identifier,
		version: u32,
	) -> Result<origin_primitives::token::TokenStateEventView<subxt::utils::H256>, OriginSdkError> {
		self.client.view()?.token().state_event(token, version).await
	}

	pub async fn resolve_pallet(&self, index: u16) -> Result<Vec<u8>, OriginSdkError> {
		self.client.view()?.token().resolve_pallet(index).await
	}
}
