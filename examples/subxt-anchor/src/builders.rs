use crate::{
	cord::runtime_types::{
		bounded_collections::bounded_vec::BoundedVec,
		cord_primitives::{
			element::{ElementType, Elum},
			identifier::Ss58Identifier,
			packet::Attributes,
		},
		pallet_entity::entity::EntityInfo,
		pallet_register::register::{AttributeFlags, AttributeSpec, LookupSpec, RegistryKind},
	},
	sample_data::{
		RegistryAttributeTemplate, SampleData, SpecTemplate, TemplateContext, TokenBindings,
		TokenSource, ValueTemplate,
	},
};
use color_eyre::eyre::{eyre, Result};
use hex::FromHex;
use rand::{rngs::StdRng, Rng, SeedableRng};
use sp_core::blake2_256;
use std::{
	collections::BTreeMap,
	time::{SystemTime, UNIX_EPOCH},
};

pub type AttributeKey = BoundedVec<u8>;
type PacketPairs = BoundedVec<(AttributeKey, Elum)>;

#[derive(Clone)]
pub struct RegistryBlueprint {
	pub info: Elum,
	pub kind: RegistryKind,
	pub attribute_schema: BoundedVec<AttributeSpec>,
	pub token_spec: LookupSpec,
	pub lookup_specs: BoundedVec<LookupSpec>,
}

pub fn build_entity_info(data: &SampleData, ctx: &TemplateContext) -> Result<EntityInfo> {
	let entity = data.entity();
	let mut extra = Vec::new();
	for (key, value) in entity.attributes.iter() {
		let rendered = ctx.render(value);
		extra.push((attribute_key(key.as_bytes()), raw_element(rendered)?));
	}
	let attributes = if extra.is_empty() { None } else { Some(build_attributes(extra)?) };

	Ok(EntityInfo {
		display: raw_element(ctx.render(&entity.display))?,
		legal: raw_element(ctx.render(&entity.legal))?,
		web: raw_element(ctx.render(&entity.web))?,
		email: raw_element(ctx.render(&entity.email))?,
		twitter: raw_element(ctx.render(&entity.twitter))?,
		attributes,
	})
}

pub fn build_registry_blueprint(
	data: &SampleData,
	ctx: &TemplateContext,
) -> Result<RegistryBlueprint> {
	let registry = data.registry();
	let info = raw_element(ctx.render(&registry.info))?;
	let kind = parse_registry_kind(&registry.kind)?;
	let attribute_schema = build_attribute_schema(&registry.attribute_schema)?;
	let token_spec = build_lookup_spec(&registry.token_spec)?;
	let lookup_specs_vec: Result<Vec<_>> =
		registry.lookup_specs.iter().map(build_lookup_spec).collect();
	let lookup_specs = BoundedVec(lookup_specs_vec?);

	Ok(RegistryBlueprint { info, kind, attribute_schema, token_spec, lookup_specs })
}

pub fn build_packet_attributes(
	data: &SampleData,
	blueprint: &RegistryBlueprint,
	entity_token: &Ss58Identifier,
	registry_token: &Ss58Identifier,
	ctx: &TemplateContext,
) -> Result<PacketPairs> {
	let bindings = TokenBindings::new(Some(entity_token), Some(registry_token));
	let mut provided: BTreeMap<Vec<u8>, Elum> = BTreeMap::new();

	for template in data.packet().attributes.iter() {
		let key_bytes = template.key.as_bytes().to_vec();
		let value = render_value(&template.value, ctx, &bindings)?;
		if provided.insert(key_bytes, value).is_some() {
			return Err(eyre!("duplicate packet attribute '{}' in sample data", template.key));
		}
	}

	let mut ordered: Vec<(AttributeKey, Elum)> = Vec::new();
	for spec in blueprint.attribute_schema.0.iter() {
		let key_bytes = spec.key.0.clone();
		let value = match provided.remove(&key_bytes) {
			Some(val) => val,
			None if flag_is_optional(&spec.flags) => Elum::None,
			None => {
				let key_str = String::from_utf8_lossy(&key_bytes).into_owned();
				return Err(eyre!("sample data missing required packet attribute '{key_str}'"));
			},
		};
		ordered.push((spec.key.clone(), value));
	}

	if let Some(extra_key) = provided.keys().next() {
		let key_str = String::from_utf8_lossy(extra_key).into_owned();
		return Err(eyre!(
			"sample data defines attribute '{key_str}' that is not in the registry schema"
		));
	}

	Ok(BoundedVec(ordered))
}

fn flag_is_optional(flags: &AttributeFlags) -> bool {
	flags.bits & 1 == 1
}

fn build_attribute_schema(
	templates: &[RegistryAttributeTemplate],
) -> Result<BoundedVec<AttributeSpec>> {
	let mut specs = Vec::new();
	for entry in templates.iter() {
		let kind = parse_element_type(&entry.element)?;
		let flags =
			if entry.optional { AttributeFlags { bits: 1 } } else { AttributeFlags { bits: 0 } };
		specs.push(AttributeSpec { key: attribute_key(entry.key.as_bytes()), kind, flags });
	}
	Ok(BoundedVec(specs))
}

fn build_lookup_spec(template: &SpecTemplate) -> Result<LookupSpec> {
	match template {
		SpecTemplate::Single { key } => {
			if key.trim().is_empty() {
				return Err(eyre!("lookup spec cannot reference an empty key"));
			}
			Ok(LookupSpec::Single(attribute_key(key.as_bytes())))
		},
		SpecTemplate::Combo { keys } => {
			if keys.is_empty() {
				return Err(eyre!("lookup combo spec must include at least one key"));
			}
			Ok(LookupSpec::Combo(bounded_bytes_list(keys.iter().map(|k| k.as_bytes()).collect())))
		},
	}
}

fn render_value(
	template: &ValueTemplate,
	ctx: &TemplateContext,
	bindings: &TokenBindings,
) -> Result<Elum> {
	match template {
		ValueTemplate::Raw { template } => raw_element(ctx.render(template)),
		ValueTemplate::Hash { template } => Ok(hash_element(ctx.render(template).as_bytes())),
		ValueTemplate::Token { source } => match source {
			TokenSource::Entity => bindings
				.entity
				.cloned()
				.map(Elum::Token)
				.ok_or_else(|| eyre!("entity token not available yet")),
			TokenSource::Registry => bindings
				.registry
				.cloned()
				.map(Elum::Token)
				.ok_or_else(|| eyre!("registry token not available yet")),
		},
		ValueTemplate::Salt { template } => {
			let seed = template
				.as_ref()
				.map(|tpl| ctx.render(tpl))
				.unwrap_or_else(|| ctx.label().to_string());
			raw_element(random_salt(&seed))
		},
		ValueTemplate::Bool { value } => Ok(Elum::Bool(if *value { 1 } else { 0 })),
		ValueTemplate::U64 { value } => Ok(Elum::U64(value.to_le_bytes())),
		ValueTemplate::U128 { value } => {
			let parsed = value.parse::<u128>().map_err(|err| eyre!("invalid u128 value: {err}"))?;
			Ok(Elum::U128(parsed.to_le_bytes()))
		},
		ValueTemplate::Cid { hex } => {
			let stripped = hex.strip_prefix("0x").unwrap_or(hex);
			let bytes = Vec::from_hex(stripped).map_err(|err| eyre!("invalid CID hex: {err}"))?;
			Ok(Elum::CID(BoundedVec(bytes)))
		},
		ValueTemplate::None => Ok(Elum::None),
	}
}

fn parse_registry_kind(input: &str) -> Result<RegistryKind> {
	match input.to_ascii_lowercase().as_str() {
		"raw" => Ok(RegistryKind::Raw),
		"token" => Ok(RegistryKind::Token),
		"hash" => Ok(RegistryKind::Hash),
		other => Err(eyre!("unsupported registry kind '{other}' in sample data")),
	}
}

fn parse_element_type(input: &str) -> Result<ElementType> {
	match input.to_ascii_lowercase().as_str() {
		"none" => Ok(ElementType::None),
		"raw" => Ok(ElementType::Raw),
		"bool" => Ok(ElementType::Bool),
		"u64" => Ok(ElementType::U64),
		"u128" => Ok(ElementType::U128),
		"hash" => Ok(ElementType::Hash),
		"token" => Ok(ElementType::Token),
		"cid" => Ok(ElementType::Cid),
		other => Err(eyre!("unsupported element type '{other}' in sample data")),
	}
}

fn build_attributes(mut pairs: Vec<(AttributeKey, Elum)>) -> Result<Attributes> {
	pairs.sort_by(|(a, _), (b, _)| a.0.cmp(&b.0));
	Ok(Attributes(BoundedVec(pairs)))
}

pub fn attribute_key(bytes: &[u8]) -> AttributeKey {
	BoundedVec(bytes.to_vec())
}

fn bounded_bytes_list(keys: Vec<&[u8]>) -> BoundedVec<AttributeKey> {
	BoundedVec(keys.into_iter().map(|k| attribute_key(k)).collect())
}

fn raw_element(value: impl AsRef<str>) -> Result<Elum> {
	Ok(Elum::Raw(BoundedVec(value.as_ref().as_bytes().to_vec())))
}

fn hash_element(input: &[u8]) -> Elum {
	Elum::Hash(blake2_256(input))
}

fn random_salt(seed_label: &str) -> String {
	let seed = blake2_256(seed_label.as_bytes());
	let mut rng = StdRng::from_seed(seed);
	let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
	format!("salt-{seed_label}-{ts}-{:04x}", rng.gen::<u16>())
}
