use crate::{
	client::Client,
	error::{Error, Result},
};
use scale_value::{Composite, Value, ValueDef};
use subxt::dynamic::{self, DecodedValueThunk};

pub struct ViewDispatch {
	pub thunk: DecodedValueThunk,
	pub metadata: subxt::Metadata,
	pub output_ty: u32,
}

pub async fn call_view(
	client: &Client,
	pallet: &str,
	function: &str,
	args: Value,
) -> Result<ViewDispatch> {
	let metadata = client.api.metadata();
	let pallet_meta = metadata
		.pallet_by_name(pallet)
		.ok_or_else(|| Error::NotFound(format!("pallet '{pallet}' not found")))?;
	let view = pallet_meta
		.view_function_by_name(function)
		.ok_or_else(|| Error::NotFound(format!("view '{pallet}.{function}' not found")))?;
	let query_id = *view.query_id();
	let output_ty = view.output_ty();
	let payload = dynamic::view_function_call(query_id, expect_composite(args)?);
	let api = client.api.view_functions().at_latest().await.map_err(Error::from)?;
	let thunk = api.call(payload).await.map_err(Error::from)?;
	let metadata = metadata.clone();
	Ok(ViewDispatch { thunk, metadata, output_ty })
}

fn expect_composite(value: Value) -> Result<Composite<()>> {
	match value.value {
		ValueDef::Composite(c) => Ok(c),
		other => Err(Error::Params(format!("expected composite value, got {other:?}"))),
	}
}
