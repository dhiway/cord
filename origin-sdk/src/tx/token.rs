use crate::{
	client::{signer::Signer, submit::TxHandle, OriginClient},
	extrinsic::calls::token,
	types::{error::OriginSdkError, token_input::TokenAttributeInput},
};
use codec::Encode;
use origin_primitives::Ss58Identifier;

pub struct TokenTx<'a, S: Signer + Clone + 'static> {
	client: &'a OriginClient,
	signer: S,
}

impl<'a, S: Signer + Clone + 'static> TokenTx<'a, S> {
	pub(crate) fn new(client: &'a OriginClient, signer: S) -> Self {
		Self { client, signer }
	}

	/// Rotate a token attribute using raw bytes.
	pub async fn submit_rotate_attribute(
		&self,
		token_id: Ss58Identifier,
		key: &[u8],
		value: &[u8],
	) -> Result<TxHandle, OriginSdkError> {
		let payload = token::rotate_attribute_call(&self.client.metadata(), token_id, key, value)?;
		self.client.submit_with(self.signer.clone()).submit_payload(payload).await
	}

	/// Convenience: accept an ElementView and SCALE-encode to bytes.
	pub async fn submit_rotate_attribute_view(
		&self,
		token_id: Ss58Identifier,
		key: &[u8],
		value: origin_primitives::element::ElementView,
	) -> Result<TxHandle, OriginSdkError> {
		let input = TokenAttributeInput::from_view(key, &value)?;
		let bytes = input.value.encode();
		self.submit_rotate_attribute(token_id, &input.key, &bytes).await
	}
}
