//! Entity flow demo: ensure an entity exists, set/rotate info, ensure nym, then print overview.
use codec::{Decode, Encode};
use origin_primitives::Ss58Identifier;
use origin_sdk::client::signer::MultiKeySigner;
use origin_sdk::client::submit::TxOutcome;
use origin_sdk::client::Signer;
use origin_sdk::domain::Domain;
use origin_sdk::types::identifiers::{account_to_ss58, ss58_to_string};
use origin_sdk::{OriginClient, OriginSdkError};
use rand::{distributions::Alphanumeric, Rng};
use scale_value::{Composite, Primitive, Value, ValueDef};
use subxt::utils::AccountId32;
use rand::distributions::Uniform;

#[tokio::main]
async fn main() -> Result<(), OriginSdkError> {
	// 1) Simple client init with sr25519 signer.
	let signer = MultiKeySigner::from_seed("//Bob", "")
		.map_err(|e| OriginSdkError::InvalidInput(e.to_string()))?;
	let client = OriginClient::connect("ws://localhost:9910", signer.clone()).await?;
	let domain = Domain::new(&client);
	let account = signer.account_id();
	let account_ss58 = account_to_ss58(&account, 29);
	println!("Using signer account: {:?} ({})", account, account_ss58);

	// 2) Ensure the account has an entity token (create if missing) and nym.
	let ensure = ensure_entity_and_nym(&client, &domain, account.clone()).await?;
	let entity_id = ensure.entity;
	let entity_id_str = ss58_to_string(&entity_id);
	println!(
		"Entity token for account {} (created: {}, nym set: {}): {}",
		account_ss58, ensure.created, ensure.nym_set, entity_id_str
	);

	// 3) Fetch the latest overview via a pure view call (no storage RPCs).
	let overview = domain.entity().overview(entity_id).await?;
	println!("Entity overview: {:?}", overview);

	Ok(())
}

struct EnsureResult {
	entity: Ss58Identifier,
	created: bool,
	nym_set: bool,
}

/// Ensure an entity exists for the account, create + set nym if not, and rotate info once.
async fn ensure_entity_and_nym(
	client: &OriginClient,
	domain: &Domain<'_>,
	account: AccountId32,
) -> Result<EnsureResult, OriginSdkError> {
	let storage_link = fetch_linked_entity_storage(client, &account).await?;
	let existing = fetch_linked_entity(client, &account).await?;
	println!(
		"Existing entity as it is (if any): {:?} {:?} (storage: {:?})",
		existing, account, storage_link
	);
	println!("Existing entity link (if any): {:?}", existing.as_ref().map(ss58_to_string));
	let mut created = false;
	let entity = if let Some(id) = existing {
		id
	} else {
		let info = demo_entity_info();
		let call = client.call().call("Entity", "set_info", vec![info]);
		match client
			.tx()
			.submit(&call.pallet, &call.function, call.args)
			.await?
			.wait_finalized()
			.await
		{
			Ok(outcome) => {
				created = true;
				let token = extract_entity_token(&outcome)
					.ok_or_else(|| OriginSdkError::View("EntityInfoSet event missing".into()))?;
				// Allow some time for linkage to appear.
				fetch_linked_entity(client, &account).await?.unwrap_or(token)
			},
			Err(e) if format!("{e}").contains("AccountAlreadyLinked") => {
				// Someone linked this account between checks; just fetch the link.
				fetch_linked_entity(client, &account).await?.ok_or_else(|| {
					OriginSdkError::View("account linked but token not retrievable".into())
				})?
			},
			Err(e) => return Err(e),
		}
	};

	// Reconfirm linkage before nym operations.
	let linked = fetch_linked_entity(client, &account).await?;
	if linked.is_none() {
		return Err(OriginSdkError::View(
			"account is not linked to any entity after set_info".into(),
		));
	}

	// Ensure nym is set.
	let current_nym = domain.entity().nym(entity.clone()).await?;
	let mut nym_set = current_nym.is_some();
	if current_nym.is_none() {
		let prefix = random_nym_prefix();
		let outcome = submit_set_entity_nym_retry(domain, &prefix).await?;
		println!("set_entity_nym finalized in block {:?}, hash {:?}", outcome.block, outcome.hash);
		nym_set = true;
	}

	// Rotate attributes/info to demonstrate updates.
	if created {
		let rotated_info = demo_entity_info();
		let outcome = domain.entity().tx().submit_set_info(rotated_info).await?;
		println!("set_info finalized in block {:?}, hash {:?}", outcome.block, outcome.hash);
	}

	Ok(EnsureResult { entity, created, nym_set })
}

/// Try setting nym, retrying once if the account linkage isn't visible yet.
async fn submit_set_entity_nym_retry(
	domain: &Domain<'_>,
	prefix: &str,
) -> Result<TxOutcome, OriginSdkError> {
	match domain.entity().tx().submit_set_entity_nym(prefix).await {
		Ok(outcome) => Ok(outcome),
		Err(e) if format!("{e}").contains("AccountNotFound") => {
			// Force a brief pause and recheck linkage before retry.
			tokio::time::sleep(std::time::Duration::from_millis(200)).await;
			let outcome = domain.entity().tx().submit_set_entity_nym(prefix).await?;
			Ok(outcome)
		},
		Err(e) if format!("{e}").contains("EntityNymAlreadySet") => {
			// Nym already present; treat as success.
			Ok(TxOutcome { block: None, hash: Default::default(), events: vec![] })
		},
		Err(e) => Err(e),
	}
}

/// Build a minimal EntityInfo payload using dynamic Value.
fn demo_entity_info() -> Value {
	let none = Value::variant("None", Composite::Unnamed(vec![]));
	let raw =
		|bytes: &[u8]| Value::variant("Raw", Composite::Unnamed(vec![Value::from_bytes(bytes)]));
	let label = random_label("entity");
	let email = format!("{}@example.com", label);
	Value::named_composite(vec![
		("display".to_owned(), raw(label.as_bytes())),
		("web".to_owned(), none.clone()),
		("email".to_owned(), raw(email.as_bytes())),
		("attributes".to_owned(), none),
	])
}

/// Extract the newly issued entity token from EntityInfoSet event fields.
fn extract_entity_token(outcome: &TxOutcome) -> Option<Ss58Identifier> {
	outcome.events.iter().find_map(|ev| {
		if ev.pallet != "Entity" || ev.variant != "EntityInfoSet" {
			return None;
		}
		ev.fields.iter().find_map(value_to_ss58)
	})
}

/// Convert common scale_value encodings back into Ss58Identifier.
fn value_to_ss58(value: &scale_value::Value) -> Option<Ss58Identifier> {
	match &value.value {
		ValueDef::Primitive(Primitive::String(s)) => Ss58Identifier::try_from(s.clone()).ok(),
		ValueDef::Composite(Composite::Unnamed(vals)) => {
			if vals.len() == 1 {
				if let Some(id) = value_to_ss58(&vals[0]) {
					return Some(id);
				}
			}
			let mut bytes = Vec::with_capacity(vals.len());
			for v in vals {
				if let ValueDef::Primitive(Primitive::U128(n)) = &v.value {
					if *n <= 255 {
						bytes.push(*n as u8);
						continue;
					}
				}
				return None;
			}
			Ss58Identifier::try_from(bytes).ok()
		},
		_ => None,
	}
}

fn random_label(prefix: &str) -> String {
	let mut rng = rand::thread_rng();
	let suffix: String = (0..6).map(|_| rng.sample(Alphanumeric) as char).collect();
	format!("{prefix}-{suffix}")
}

/// Generate a nym prefix that passes pallet validation: lowercase a-z0-9, no dots, length small.
fn random_nym_prefix() -> String {
	let mut rng = rand::thread_rng();
	let charset = b"abcdefghijklmnopqrstuvwxyz0123456789";
	let dist = Uniform::from(0..charset.len());
	let len = 8; // safe against MaxEntityNymLength once suffix is added
	let mut s = String::with_capacity(len);
	for _ in 0..len {
		s.push(char::from(charset[rng.sample(dist)]));
	}
	s
}

/// Fetch account->entity link and verify it resolves via overview; otherwise fall back to storage.
async fn fetch_linked_entity(
	client: &OriginClient,
	account: &AccountId32,
) -> Result<Option<Ss58Identifier>, OriginSdkError> {
	// 1) Try view path first
	if let Ok(link) = client.view().entity().account_token(account.clone()).await {
		if let Some(id) = link.clone() {
			if client.entity().overview(id.clone()).await.is_ok() {
				return Ok(Some(id));
			}
			eprintln!("view token invalid for overview, falling back to storage: {:?}", id);
		}
	}

	// 2) Fallback: storage path (which we know is correct)
	let storage_link = fetch_linked_entity_storage(client, account).await?;
	Ok(storage_link)
}

// /// Fetch account->entity link and verify it resolves via overview; otherwise treat as None.
// async fn fetch_linked_entity(
// 	client: &OriginClient,
// 	account: &AccountId32,
// ) -> Result<Option<Ss58Identifier>, OriginSdkError> {
// 	let link = client.view().entity().account_token(account.clone()).await?;
// 	if let Some(id) = link.clone() {
// 		if client.entity().overview(id.clone()).await.is_ok() {
// 			return Ok(Some(id));
// 		}
// 		eprintln!("token returned but overview failed: {:?}", id);
// 	}
// 	Ok(None)
// }

/// Debug helper: read EntityTokenOfAccount directly from storage to confirm mapping.
async fn fetch_linked_entity_storage(
	client: &OriginClient,
	account: &AccountId32,
) -> Result<Option<Ss58Identifier>, OriginSdkError> {
	use subxt::dynamic::Value;
	let address = subxt::dynamic::storage(
		"Entity",
		"EntityTokenOfAccount",
		vec![Value::from_bytes(account.encode())],
	);
	let api = client.online();
	let value = api
		.storage()
		.at_latest()
		.await
		.map_err(|e| OriginSdkError::View(e.to_string()))?
		.fetch(&address)
		.await
		.map_err(|e| OriginSdkError::View(e.to_string()))?;
	if let Some(val) = value {
		let bytes = val.encoded();
		let id = Ss58Identifier::decode(&mut &bytes[..])
			.map_err(|e| OriginSdkError::Decode(e.to_string()))?;
		return Ok(Some(id));
	}
	Ok(None)
}
