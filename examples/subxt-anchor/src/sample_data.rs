use crate::cord::runtime_types::cord_primitives::identifier::Ss58Identifier;
use color_eyre::eyre::{Result, WrapErr};
use serde::Deserialize;
use std::{collections::BTreeMap, fs, path::Path};

#[derive(Debug, Deserialize)]
pub struct SampleData {
	entity: EntityTemplate,
	registry: RegistryTemplate,
	packet: PacketTemplate,
}

impl SampleData {
	pub fn load(path: impl AsRef<Path>) -> Result<Self> {
		let path = path.as_ref();
		let contents = fs::read_to_string(path)
			.wrap_err_with(|| format!("failed to read sample-data file {}", path.display()))?;
		let data = serde_json::from_str::<SampleData>(&contents)
			.wrap_err_with(|| format!("failed to parse sample-data JSON {}", path.display()))?;
		Ok(data)
	}

	pub fn entity(&self) -> &EntityTemplate {
		&self.entity
	}

	pub fn registry(&self) -> &RegistryTemplate {
		&self.registry
	}

	pub fn packet(&self) -> &PacketTemplate {
		&self.packet
	}
}

#[derive(Debug, Deserialize)]
pub struct EntityTemplate {
	pub display: String,
	pub legal: String,
	pub web: String,
	pub email: String,
	pub twitter: String,
	#[serde(default)]
	pub attributes: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub struct RegistryTemplate {
	pub info: String,
	pub kind: String,
	#[serde(rename = "attribute_schema")]
	pub attribute_schema: Vec<RegistryAttributeTemplate>,
	pub token_spec: SpecTemplate,
	pub lookup_specs: Vec<SpecTemplate>,
}

#[derive(Debug, Deserialize)]
pub struct RegistryAttributeTemplate {
	pub key: String,
	#[serde(rename = "type")]
	pub element: String,
	#[serde(default)]
	pub optional: bool,
}

#[derive(Debug, Deserialize)]
pub struct PacketTemplate {
	pub attributes: Vec<PacketAttributeTemplate>,
}

#[derive(Debug, Deserialize)]
pub struct PacketAttributeTemplate {
	pub key: String,
	pub value: ValueTemplate,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SpecTemplate {
	Single { key: String },
	Combo { keys: Vec<String> },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ValueTemplate {
	Raw {
		template: String,
	},
	Hash {
		template: String,
	},
	Token {
		source: TokenSource,
	},
	Salt {
		#[serde(default)]
		template: Option<String>,
	},
	Bool {
		value: bool,
	},
	U64 {
		value: u64,
	},
	U128 {
		value: String,
	},
	Cid {
		hex: String,
	},
	None,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenSource {
	Entity,
	Registry,
}

#[derive(Clone, Debug)]
pub struct TemplateContext {
	label: String,
	base_label: String,
	run_id: String,
}

impl TemplateContext {
	pub fn new(
		base_label: impl Into<String>,
		run_id: impl Into<String>,
		label: impl Into<String>,
	) -> Self {
		Self { base_label: base_label.into(), run_id: run_id.into(), label: label.into() }
	}

	pub fn label(&self) -> &str {
		&self.label
	}

	pub fn render(&self, template: &str) -> String {
		template
			.replace("{label}", &self.label)
			.replace("{base_label}", &self.base_label)
			.replace("{run_id}", &self.run_id)
	}
}

pub struct TokenBindings<'a> {
	pub entity: Option<&'a Ss58Identifier>,
	pub registry: Option<&'a Ss58Identifier>,
}

impl<'a> TokenBindings<'a> {
	pub fn new(entity: Option<&'a Ss58Identifier>, registry: Option<&'a Ss58Identifier>) -> Self {
		Self { entity, registry }
	}
}
