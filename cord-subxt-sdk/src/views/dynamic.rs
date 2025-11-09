use crate::{
	client::Client,
	error::{Error, Result},
};

pub fn method_name(pallet: &str, function: &str) -> String {
	format!("{pallet}_{function}")
}

pub fn encode_args_from_json(
	_pallet: &str,
	_function: &str,
	_args: &serde_json::Value,
	_meta: &subxt::Metadata,
) -> Result<Vec<u8>> {
	Err(Error::Params("JSON view encoding not yet implemented".into()))
}

pub async fn state_call_json(
	_client: &Client,
	pallet: &str,
	function: &str,
	json_args: serde_json::Value,
) -> Result<serde_json::Value> {
	let _ = (pallet, function, json_args);
	Err(Error::NotFound("dynamic state_call not wired".into()))
}
