use crate::{
	client::{signer::Signer, OriginClient, ViewClient},
	types::{error::OriginSdkError, TokenLookupView, TokenTimelineView},
};
use origin_primitives::Ss58Identifier;

pub struct TokenClientWithSigner<'a, S: Signer + Clone + 'static> {
	client: &'a OriginClient,
	signer: S,
}

impl<'a, S: Signer + Clone + 'static> TokenClientWithSigner<'a, S> {
	pub(crate) fn new(client: &'a OriginClient, signer: S) -> Self {
		Self { client, signer }
	}

	fn view(&self) -> ViewClient {
		self.client.view_with(self.signer.clone())
	}

	pub async fn timeline(
		&self,
		token: Ss58Identifier,
		start: Option<u32>,
		limit: Option<u32>,
	) -> Result<TokenTimelineView, OriginSdkError> {
		self.view().token().timeline(token, start, limit).await
	}

	pub async fn resolve_identifier(
		&self,
		token: Ss58Identifier,
	) -> Result<TokenLookupView, OriginSdkError> {
		self.view().token().resolve_identifier(token).await
	}

	pub async fn maybe_resolve_identifier(
		&self,
		token: Ss58Identifier,
	) -> Result<Option<TokenLookupView>, OriginSdkError> {
		self.view().token().maybe_resolve_identifier(token).await
	}

	pub async fn pallet_index_of(&self, name: Vec<u8>) -> Result<u16, OriginSdkError> {
		self.view().token().pallet_index_of(name).await
	}

	pub async fn pallet_name(&self, index: u16) -> Result<String, OriginSdkError> {
		self.view().token().pallet_name(index).await
	}

	pub async fn next_pallet_index(&self) -> Result<u16, OriginSdkError> {
		self.view().token().next_pallet_index().await
	}

	pub async fn genesis_network_id(&self) -> Result<u32, OriginSdkError> {
		self.view().token().genesis_network_id().await
	}

	pub async fn state_version(&self, token: Ss58Identifier) -> Result<u32, OriginSdkError> {
		self.view().token().state_version(token).await
	}

	pub async fn state_event(
		&self,
		token: Ss58Identifier,
		version: u32,
	) -> Result<origin_primitives::token::TokenStateEventView<subxt::utils::H256>, OriginSdkError>
	{
		self.view().token().state_event(token, version).await
	}

	pub async fn maybe_state_event(
		&self,
		token: Ss58Identifier,
		version: u32,
	) -> Result<
		Option<origin_primitives::token::TokenStateEventView<subxt::utils::H256>>,
		OriginSdkError,
	> {
		self.view().token().maybe_state_event(token, version).await
	}

	pub async fn resolve_pallet(&self, index: u16) -> Result<Vec<u8>, OriginSdkError> {
		self.view().token().resolve_pallet(index).await
	}
}
