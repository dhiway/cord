use anyhow::Result;
use bs58;
use hex;
use origin_primitives::{
	packet::ElementType,
	registry::{
		LookupSpecView, RegistryAttributeView, RegistryInfoView, RegistryKind, RegistryStatus,
	},
	view::ElementView,
};
use serde_json::json;

use crate::demo::util::ViewStyle;

pub fn render_registry_snapshot_cli(
	registry_ss58: &str,
	details: &RegistryInfoView,
	lookups: &[LookupSpecView],
	timeline: &[crate::types::token::StateEventRecord],
	next_cursor: Option<u32>,
	style: ViewStyle,
	output_json: bool,
) -> Result<()> {
	if output_json {
		let json = json!({
			"registry": registry_ss58,
			"maintainer": crate::demo::ss58_string(&details.maintainer),
			"details": details,
			"lookupSpecs": lookups,
			"timeline": timeline,
			"nextCursor": next_cursor,
		});
		println!("{}", serde_json::to_string_pretty(&json)?);
		return Ok(());
	}

	println!("\n🗂️ Registry Snapshot");
	let maintainer = crate::demo::ss58_string(&details.maintainer);
	println!("  ↳ • registry   : {registry_ss58}");
	println!("  ↳ • maintainer : {maintainer}");
	println!("  ↳ • kind       : {}", describe_registry_kind(&details.kind));
	println!("  ↳ • status     : {}", describe_registry_status(&details.status));
	println!("\n📝 Info\n  {}", describe_element(&details.info));
	println!("\n🔑 Token Spec\n  {}", describe_lookup(&details.token_spec));
	print_attribute_schema(&details.attributes, style.is_full());
	print_lookup_specs(lookups, style.is_full());
	print_registry_timeline(timeline, next_cursor, style.is_full());
	Ok(())
}

fn print_attribute_schema(attributes: &[RegistryAttributeView], full: bool) {
	println!("\n🔣 Attribute Schema");
	if attributes.is_empty() {
		println!("  ↳ • (none)");
		return;
	}
	let mut shown = 0usize;
	let limit = if full { attributes.len() } else { attributes.len().min(8) };
	for attr in attributes.iter().take(limit) {
		let key = key_to_label(&attr.key);
		println!(
			"  ↳ • {:<16} {}{}",
			key,
			describe_element_type(&attr.kind),
			if attr.optional { " [optional]" } else { "" }
		);
		shown += 1;
	}
	if !full && attributes.len() > shown {
		println!("    … {} more", attributes.len() - shown);
	}
}

fn print_lookup_specs(specs: &[LookupSpecView], full: bool) {
	println!("\n🔍 Lookup Specs");
	if specs.is_empty() {
		println!("  ↳ • (none)");
		return;
	}
	let mut shown = 0usize;
	let limit = if full { specs.len() } else { specs.len().min(5) };
	for spec in specs.iter().take(limit) {
		println!("  ↳ • {}", describe_lookup(spec));
		shown += 1;
	}
	if !full && specs.len() > shown {
		println!("    … {} more", specs.len() - shown);
	}
}

fn print_registry_timeline(
	records: &[crate::types::token::StateEventRecord],
	next_cursor: Option<u32>,
	full: bool,
) {
	println!("\n⏱️ Token Timeline");
	if records.is_empty() {
		println!("  ↳ • (no events)");
	} else {
		let limit = if full { records.len() } else { records.len().min(8) };
		for (idx, event) in records.iter().take(limit).enumerate() {
			let action_utf8 = String::from_utf8(event.action.clone())
				.unwrap_or_else(|_| format!("0x{}", hex::encode(&event.action)));
			println!(
				"  ↳ • #{:<2} action={:<24} block=#{} extrinsic={} digest=0x{}",
				idx + 1,
				action_utf8,
				event.seal.height,
				event.seal.index,
				hex::encode(event.digest)
			);
		}
		if !full && records.len() > limit {
			println!("    … {} more", records.len() - limit);
		}
	}
	if let Some(cursor) = next_cursor {
		println!("  ↳ • next cursor: {cursor}");
	}
}

fn describe_lookup(spec: &LookupSpecView) -> String {
	match spec {
		LookupSpecView::Single(key) => format!("single:{}", key_to_label(key)),
		LookupSpecView::Combo(keys) => {
			let joined = keys.iter().map(|k| key_to_label(k)).collect::<Vec<_>>().join(", ");
			format!("combo:[{}]", joined)
		},
	}
}

fn describe_element(view: &ElementView) -> String {
	match view {
		ElementView::None => "(none)".into(),
		ElementView::Bool(value) => format!("bool:{value}"),
		ElementView::U64(value) => format!("u64:{value}"),
		ElementView::U128(value) => format!("u128:{value}"),
		ElementView::Hash(bytes) => format!("hash:0x{}", hex::encode(bytes)),
		ElementView::Token(token) => format!("token:{}", crate::demo::ss58_string(token)),
		ElementView::Cid(bytes) => format!("cid:{}", bs58::encode(bytes).into_string()),
		ElementView::Raw(bytes) => {
			let text = String::from_utf8(bytes.clone())
				.unwrap_or_else(|_| format!("0x{}", hex::encode(bytes)));
			format!("raw:{text}")
		},
	}
}

fn key_to_label(bytes: &[u8]) -> String {
	String::from_utf8(bytes.to_vec()).unwrap_or_else(|_| format!("0x{}", hex::encode(bytes)))
}

fn describe_registry_kind(kind: &RegistryKind) -> &'static str {
	match kind {
		RegistryKind::Raw => "Raw",
		RegistryKind::Token => "Token",
		RegistryKind::Hash => "Hash",
	}
}

fn describe_registry_status(status: &RegistryStatus) -> &'static str {
	match status {
		RegistryStatus::Active => "Active",
		RegistryStatus::Revoked => "Revoked",
		RegistryStatus::Deleted => "Deleted",
	}
}

fn describe_element_type(kind: &ElementType) -> &'static str {
	match kind {
		ElementType::None => "None",
		ElementType::Raw => "Raw",
		ElementType::Bool => "Bool",
		ElementType::U64 => "U64",
		ElementType::U128 => "U128",
		ElementType::Hash => "Hash",
		ElementType::Token => "Token",
		ElementType::Cid => "Cid",
	}
}
