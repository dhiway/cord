use std::sync::Arc;

use super::builder::DynamicCall;
use crate::client::connection::Connection;
use crate::client::Signer;
use crate::types::error::OriginSdkError;

/// Meta-transaction helper (scaffold).
#[derive(Clone)]
pub struct MetaTxClient {
	#[allow(dead_code)]
	connection: Arc<Connection>,
	signer: Arc<dyn Signer>,
}

impl MetaTxClient {
	pub(crate) fn new(connection: Arc<Connection>, signer: Arc<dyn Signer>) -> Self {
		Self { connection, signer }
	}

	/// Wrap a call into MetaTx::submit(pallet, function, args_bytes).
	pub fn wrap(&self, call: DynamicCall) -> Result<DynamicCall, OriginSdkError> {
		let inner_value = subxt::dynamic::tx(call.pallet, call.function, call.args).into_value();
		let wrapped = DynamicCall {
			pallet: "MetaTx".into(),
			function: "submit".into(),
			args: vec![inner_value],
		};
		Ok(wrapped)
	}

	/// Sign and submit a meta-transaction via MetaTx pallet.
	pub async fn sign_and_submit(
		&self,
		call: DynamicCall,
	) -> Result<crate::client::submit::TxHandle, OriginSdkError> {
		let wrapped = self.wrap(call)?;
		let payload = subxt::dynamic::tx(wrapped.pallet, wrapped.function, wrapped.args);
		let submit =
			crate::client::submit::SubmitClient::new(self.connection.clone(), self.signer.clone());
		submit.submit_payload(payload).await
	}
}
