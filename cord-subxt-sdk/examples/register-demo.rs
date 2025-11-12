use anyhow::{Context, Result};
use cord_primitives::view_api::{RegisterDetailsRequest, RegisterLookupSpecsRequest};
use origin::{
	demo,
	params::config::CordConfig,
	query::auth::{AuthorizationBuilder, SignatureScheme},
	tx,
};

#[tokio::main]
async fn main() -> Result<()> {
	let label = demo::random_label("register-demo");
	let client = origin::Client::connect("ws://127.0.0.1:9944", origin::ChainFlavor::Auto).await?;
	let signer = tx::signer::dev_alice();
	let account_id =
		<tx::signer::sr25519::Keypair as subxt::tx::Signer<CordConfig>>::account_id(&signer);
	let signed_auth = AuthorizationBuilder::from_signer(&signer, SignatureScheme::Sr25519, None)
		.context("failed to build view authorization")?;
	let authorization = signed_auth.as_request().context("failed to convert view authorization")?;

	let profile = demo::entity_profile(&label);
	let (entity_token, created) =
		demo::ensure_entity_token(&client, &signer, &authorization, &account_id, &profile).await?;
	if created {
		println!("Set entity profile for Alice (token {entity_token})");
	} else {
		println!("Reusing existing entity token {entity_token}");
	}
	println!("Maintainer entity token {entity_token}");
	let registry_spec = demo::registry_blueprint(&label);
	let registry_id = demo::create_registry(&client, &signer, registry_spec).await?;
	println!("Minted registry token {}\n", demo::ss58_string(&registry_id));
	let registry_ident = registry_id.clone();

	let info_req =
		RegisterDetailsRequest { auth: authorization.clone(), registry: registry_ident.clone() };
	let info = client.query().register().details(&info_req).await?;
	println!("registry info:\n{}", serde_json::to_string_pretty(&info)?);

	let _schema = client.query().register().schema(&info_req).await?;
	println!("\nattribute schema ({} keys):", info.attributes.len());
	for attr in info.attributes {
		let key = String::from_utf8_lossy(&attr.key);
		println!("- {} ({:?}){}", key, attr.kind, if attr.optional { " [optional]" } else { "" });
	}

	let lookup_req = RegisterLookupSpecsRequest {
		auth: authorization.clone(),
		registry: registry_ident.clone(),
	};
	let lookups = client.query().register().lookup_specs(&lookup_req).await?;
	println!("\nlookup specs: {lookups:?}");
	Ok(())
}
