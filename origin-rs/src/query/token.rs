use crate::{
	client::{signer::OriginSigner, OriginClient},
	types::{error::OriginSdkError, TokenLookupView, TokenStateEventViewSdk, TokenTimelineViewSdk},
};
use origin_primitives::Ss58Identifier;

type Auth = origin_primitives::Authorization<
	origin_primitives::AccountId,
	Vec<u8>,
	origin_primitives::Signature,
>;

pub struct TokenClientWithSigner<'a> {
	client: &'a OriginClient,
	signer: OriginSigner,
}

impl<'a> TokenClientWithSigner<'a> {
	pub(crate) fn new(client: &'a OriginClient, signer: OriginSigner) -> Self {
		Self { client, signer }
	}

	fn view(&self) -> crate::client::ViewClient {
		self.client.view()
	}

	async fn auth(&self, function: &str) -> Result<Auth, OriginSdkError> {
		self.view().authorization_for(&self.signer, "Token", function).await
	}

	pub async fn timeline(
		&self,
		token: Ss58Identifier,
		start: Option<u32>,
		limit: Option<u32>,
	) -> Result<Option<TokenTimelineViewSdk>, OriginSdkError> {
		let auth = self.auth("timeline").await?;
		self.view().call("Token", "timeline", (auth, token, start, limit)).await
	}

	pub async fn resolve_identifier(
		&self,
		token: Ss58Identifier,
	) -> Result<Option<TokenLookupView>, OriginSdkError> {
		let auth = self.auth("resolve_identifier").await?;
		self.view().call("Token", "resolve_identifier", (auth, token)).await
	}

	pub async fn pallet_index_of(&self, name: Vec<u8>) -> Result<Option<u16>, OriginSdkError> {
		let auth = self.auth("pallet_index_of").await?;
		self.view().call("Token", "pallet_index_of", (auth, name)).await
	}

	pub async fn pallet_name(&self, index: u16) -> Result<Option<String>, OriginSdkError> {
		let auth = self.auth("pallet_name_view").await?;
		self.view().call("Token", "pallet_name_view", (auth, index)).await
	}

	pub async fn next_pallet_index(&self) -> Result<Option<u16>, OriginSdkError> {
		let auth = self.auth("next_pallet_index").await?;
		self.view().call("Token", "next_pallet_index", auth).await
	}

	pub async fn genesis_network_id(&self) -> Result<Option<u16>, OriginSdkError> {
		let auth = self.auth("genesis_network_id").await?;
		self.view().call("Token", "genesis_network_id", auth).await
	}

	pub async fn state_version(
		&self,
		token: Ss58Identifier,
	) -> Result<Option<u32>, OriginSdkError> {
		let auth = self.auth("state_version").await?;
		self.view().call("Token", "state_version", (auth, token)).await
	}

	pub async fn state_event(
		&self,
		token: Ss58Identifier,
		version: u32,
	) -> Result<Option<TokenStateEventViewSdk>, OriginSdkError> {
		let auth = self.auth("state_event").await?;
		self.view().call("Token", "state_event", (auth, token, version)).await
	}

	pub async fn has_history(&self, token: Ss58Identifier) -> Result<bool, OriginSdkError> {
		let auth = self.auth("has_history").await?;
		self.view().call("Token", "has_history", (auth, token)).await
	}

	pub async fn latest_state_event(
		&self,
		token: Ss58Identifier,
	) -> Result<Option<TokenStateEventViewSdk>, OriginSdkError> {
		let auth = self.auth("latest_state_event").await?;
		self.view().call("Token", "latest_state_event", (auth, token)).await
	}

	pub async fn recent_timeline(
		&self,
		token: Ss58Identifier,
		limit: Option<u32>,
	) -> Result<Option<Vec<TokenStateEventViewSdk>>, OriginSdkError> {
		let auth = self.auth("recent_timeline").await?;
		self.view().call("Token", "recent_timeline", (auth, token, limit)).await
	}

	pub async fn resolve_pallet(&self, index: u16) -> Result<Option<String>, OriginSdkError> {
		let auth = self.auth("resolve_pallet").await?;
		self.view().call("Token", "resolve_pallet", (auth, index)).await
	}
}
