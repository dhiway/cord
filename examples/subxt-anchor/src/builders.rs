use crate::cord::runtime_types::{
	bounded_collections::bounded_vec::BoundedVec,
	cord_primitives::{
		element::{ElementType, Elum},
		identifier::Ss58Identifier,
		packet::Attributes,
	},
	pallet_entity::entity::EntityInfo,
	pallet_register::register::{AttributeFlags, AttributeSpec, LookupSpec, RegistryKind},
};
use color_eyre::eyre::Result;
use rand::{rngs::StdRng, Rng, SeedableRng};
use sp_core::blake2_256;
use std::time::{SystemTime, UNIX_EPOCH};

type AttributeKey = BoundedVec<u8>;
type PacketPairs = BoundedVec<(AttributeKey, Elum)>;

pub struct RegistryBlueprint {
	pub info: Elum,
	pub kind: RegistryKind,
	pub attribute_schema: BoundedVec<AttributeSpec>,
	pub token_spec: LookupSpec,
	pub lookup_specs: BoundedVec<LookupSpec>,
	pub packet_seed: Vec<(AttributeKey, Elum)>,
}

pub fn build_entity_info(label: &str) -> Result<EntityInfo> {
	Ok(EntityInfo {
		display: raw_element(format!("{label} showcase"))?,
		legal: raw_element("CORD Showcase LLP")?,
		web: raw_element(format!("https://{label}.cord.dev/demo"))?,
		email: raw_element("ops@cord.dev")?,
		twitter: raw_element("@cord_demo")?,
		attributes: Some(build_attributes(vec![
			("support", raw_element("support@cord.dev")?),
			("website", raw_element(format!("https://{label}.cord.dev"))?),
		])?),
	})
}

pub fn build_registry_blueprint(label: &str) -> Result<RegistryBlueprint> {
	let info = raw_element(format!("CORD registry for {label}"))?;
	let attribute_schema = BoundedVec(vec![
		spec("record_id", ElementType::Raw),
		spec("payload_hash", ElementType::Hash),
		spec("controller", ElementType::Token),
		spec("payload_salt", ElementType::Raw),
	]);

	let token_keys = bounded_bytes_list(&[b"record_id", b"controller"]);
	let lookup_specs = BoundedVec(vec![
		LookupSpec::Combo(token_keys.clone()),
		LookupSpec::Single(attribute_key(b"payload_hash")),
	]);

	let packet_seed = vec![
		(attribute_key(b"record_id"), raw_element(format!("{label}-packet"))?),
		(attribute_key(b"payload_hash"), hash_element(label.as_bytes())),
	];

	Ok(RegistryBlueprint {
		info,
		kind: RegistryKind::Raw,
		attribute_schema,
		token_spec: LookupSpec::Combo(token_keys),
		lookup_specs,
		packet_seed,
	})
}

pub fn build_packet_attributes(
	blueprint: &RegistryBlueprint,
	entity_token: &Ss58Identifier,
	label: &str,
) -> Result<PacketPairs> {
	let mut pairs = blueprint.packet_seed.clone();
	pairs.push((attribute_key(b"controller"), Elum::Token(entity_token.clone())));
	pairs.push((attribute_key(b"payload_salt"), raw_element(random_salt(label))?));
	Ok(BoundedVec(pairs))
}

fn spec(key: &str, kind: ElementType) -> AttributeSpec {
	AttributeSpec { key: attribute_key(key.as_bytes()), kind, flags: AttributeFlags { bits: 0 } }
}

fn build_attributes(mut pairs: Vec<(&str, Elum)>) -> Result<Attributes> {
	pairs.sort_by(|(a, _), (b, _)| a.as_bytes().cmp(b.as_bytes()));
	let tuples: Vec<(AttributeKey, Elum)> = pairs
		.into_iter()
		.map(|(name, elem)| (attribute_key(name.as_bytes()), elem))
		.collect();
	Ok(Attributes(BoundedVec(tuples)))
}

fn attribute_key(bytes: &[u8]) -> AttributeKey {
	BoundedVec(bytes.to_vec())
}

fn bounded_bytes_list(keys: &[&[u8]]) -> BoundedVec<AttributeKey> {
	BoundedVec(keys.iter().map(|k| attribute_key(k)).collect())
}

fn raw_element(value: impl AsRef<str>) -> Result<Elum> {
	Ok(Elum::Raw(BoundedVec(value.as_ref().as_bytes().to_vec())))
}

fn hash_element(input: &[u8]) -> Elum {
	Elum::Hash(blake2_256(input))
}

fn random_salt(label: &str) -> String {
	let seed = blake2_256(label.as_bytes());
	let mut rng = StdRng::from_seed(seed);
	let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
	format!("salt-{label}-{ts}-{:04x}", rng.gen::<u16>())
}
