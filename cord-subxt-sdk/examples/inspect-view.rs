use oc::{client::Client, flavors::ChainFlavor};
use subxt::Metadata;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let client = Client::connect("ws://127.0.0.1:9944", ChainFlavor::Auto).await?;
	let metadata = client.metadata();
	print_view(&metadata, "Entity", "account_token");
	print_view(&metadata, "Entity", "details");
	print_view(&metadata, "Register", "details");
	Ok(())
}

fn print_view(metadata: &Metadata, pallet: &str, function: &str) {
	let pallet_meta = metadata.pallet_by_name(pallet).expect("pallet");
	let view = pallet_meta.view_function_by_name(function).expect("view");
	let ty_id = view.output_ty();
	let ty = metadata.types().resolve(ty_id).expect("type");
	println!("{pallet}.{function} -> {:?}", ty.path);
	println!("  type id: {ty_id}");
	println!("  type def: {:?}\n", ty.type_def);
}
