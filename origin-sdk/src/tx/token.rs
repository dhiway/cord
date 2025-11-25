use crate::{
	client::{signer::Signer, submit::TxOutcome, OriginClient},
	extrinsic::calls::token,
	types::{error::OriginSdkError, token_input::TokenAttributeInput},
};
use codec::Encode;
use origin_primitives::Ss58Identifier;

pub struct TokenTx<'a> {
	client: &'a OriginClient,
}

impl<'a> TokenTx<'a> {
	pub(crate) fn new(client: &'a OriginClient) -> Self {
		Self { client }
	}

	pub fn using<S>(&self, signer: S) -> TokenTxWithSigner<'a, S>
	where
		S: Signer + Clone + 'static,
	{
		TokenTxWithSigner::new(self.client, signer)
	}

	/// Rotate a token attribute using raw bytes (typed helpers TBD).
	pub async fn submit_rotate_attribute(
		&self,
		token_id: Ss58Identifier,
		key: &[u8],
		value: &[u8],
	) -> Result<TxOutcome, OriginSdkError> {
		let payload = token::rotate_attribute_call(&self.client.metadata(), token_id, key, value)?;
		self.client.tx()?.submit_payload(payload).await?.wait_in_block().await
	}

	/// Convenience: accept an ElementView and SCALE-encode to bytes.
	pub async fn submit_rotate_attribute_view(
		&self,
		token_id: Ss58Identifier,
		key: &[u8],
		value: origin_primitives::element::ElementView,
	) -> Result<TxOutcome, OriginSdkError> {
		let input = TokenAttributeInput::from_view(key, &value)?;
		let bytes = input.value.encode();
		self.submit_rotate_attribute(token_id, &input.key, &bytes).await
	}
}

pub struct TokenTxWithSigner<'a, S: Signer + Clone + 'static> {
	client: &'a OriginClient,
	signer: S,
}

impl<'a, S: Signer + Clone + 'static> TokenTxWithSigner<'a, S> {
	pub(crate) fn new(client: &'a OriginClient, signer: S) -> Self {
		Self { client, signer }
	}

	pub async fn submit_rotate_attribute(
		&self,
		token_id: Ss58Identifier,
		key: &[u8],
		value: &[u8],
	) -> Result<TxOutcome, OriginSdkError> {
		let payload = token::rotate_attribute_call(&self.client.metadata(), token_id, key, value)?;
		self.client
			.tx_with(self.signer.clone())
			.submit_payload(payload)
			.await?
			.wait_in_block()
			.await
	}

	pub async fn submit_rotate_attribute_view(
		&self,
		token_id: Ss58Identifier,
		key: &[u8],
		value: origin_primitives::element::ElementView,
	) -> Result<TxOutcome, OriginSdkError> {
		let input = TokenAttributeInput::from_view(key, &value)?;
		let bytes = input.value.encode();
		self.submit_rotate_attribute(token_id, &input.key, &bytes).await
	}
}
