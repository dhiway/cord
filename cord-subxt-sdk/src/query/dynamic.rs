use crate::{
	client::Client,
	error::{Error, Result},
};
use scale_value::{Composite, Value, ValueDef};
use subxt::dynamic::{self, DecodedValueThunk};

pub fn encode_args_from_json(
	_pallet: &str,
	_function: &str,
	args: &serde_json::Value,
	_meta: &subxt::Metadata,
) -> Result<Value> {
	scale_value::serde::to_value(args.clone())
		.map_err(|e| Error::Params(format!("json→value conversion failed: {e}")))
}

pub async fn call_view(
	client: &Client,
	pallet: &str,
	function: &str,
	args: Value,
) -> Result<DecodedValueThunk> {
	let metadata = client.api.metadata();
	let pallet_meta = metadata
		.pallet_by_name(pallet)
		.ok_or_else(|| Error::NotFound(format!("pallet '{pallet}' not found")))?;
	let view = pallet_meta
		.view_function_by_name(function)
		.ok_or_else(|| Error::NotFound(format!("view '{pallet}.{function}' not found")))?;
	let payload = dynamic::view_function_call(*view.query_id(), expect_composite(args)?);
	let api = client.api.view_functions().at_latest().await.map_err(Error::from)?;
	api.call(payload).await.map_err(Error::from)
}

fn expect_composite(value: Value) -> Result<Composite<()>> {
	match value.value {
		ValueDef::Composite(c) => Ok(c),
		other => Err(Error::Params(format!("expected composite value, got {other:?}"))),
	}
}
