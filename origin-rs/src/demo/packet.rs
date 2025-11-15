use crate::{demo::util::ViewStyle, types::token::StateEventRecord};
use origin_primitives::{
	registry::RegistryStatus,
	view::{DevAttr, DevElement, DevPacketSnapshot},
};
use serde_json::json;

pub fn render_packet_snapshot_cli(
	registry_ss58: &str,
	packet_ss58: &str,
	delegate_ss58: Option<&str>,
	snapshot: &DevPacketSnapshot<RegistryStatus>,
	timeline: &[StateEventRecord],
	next_cursor: Option<u32>,
	style: ViewStyle,
	output_json: bool,
) {
	if output_json {
		let json = json!({
			"registry": registry_ss58,
			"packet": packet_ss58,
			"delegate": delegate_ss58,
			"snapshot": snapshot,
			"timeline": timeline,
			"nextCursor": next_cursor,
		});
		println!("{}", serde_json::to_string_pretty(&json).expect("json"));
		return;
	}

	println!("\n🧾 Packet Snapshot");
	println!("  ↳ • Packet     : {packet_ss58}");
	println!("  ↳ • Registry   : {registry_ss58}");
	let delegate_line = delegate_ss58.unwrap_or("(delegate not resolved)");
	println!("  ↳ • Delegate   : {delegate_line}");
	println!("  ↳ • Controller : {}", snapshot.state.controller_ss58);
	println!(
		"  ↳ • Status     : {} (registry {})",
		describe_packet_status(&snapshot.state.status),
		describe_registry_status(&snapshot.registry_status)
	);
	println!("  ↳ • Version    : {}", snapshot.state.version);
	println!("  ↳ • Attr Hash  : {}", snapshot.state.attributes_hash_hex);
	print_packet_attributes(&snapshot.state.attributes, style.is_full());
	print_timeline(timeline, next_cursor, style.is_full());
}

fn print_packet_attributes(attrs: &[DevAttr], full: bool) {
	println!("\n🧬 Packet Attributes");
	if attrs.is_empty() {
		println!("  ↳ • (none)");
		return;
	}
	let mut shown = 0usize;
	let limit = if full { attrs.len() } else { attrs.len().min(6) };
	for attr in attrs.iter().take(limit) {
		let key = attr.key_utf8.clone().unwrap_or_else(|| attr.key_hex.clone());
		println!("  ↳ • {:<18} : {}", key, describe_dev_element(&attr.value));
		shown += 1;
	}
	if !full && attrs.len() > shown {
		println!("    … {} more", attrs.len() - shown);
	}
}

fn describe_dev_element(value: &DevElement) -> String {
	match value {
		DevElement::None => "(none)".into(),
		DevElement::Bool(flag) => format!("bool:{flag}"),
		DevElement::U64(v) => format!("u64:{v}"),
		DevElement::U128(v) => format!("u128:{v}"),
		DevElement::HashHex(hex_str) => format!("hash:{hex_str}"),
		DevElement::TokenSs58(token) => format!("token:{token}"),
		DevElement::CidBase58(cid) => format!("cid:{cid}"),
		DevElement::RawBase64(data) => format!("raw(base64):{data}"),
	}
}

fn describe_packet_status(status: &origin_primitives::packet::PacketStatus) -> &'static str {
	match status {
		origin_primitives::packet::PacketStatus::Active => "Active",
		origin_primitives::packet::PacketStatus::Revoked => "Revoked",
		origin_primitives::packet::PacketStatus::Deleted => "Deleted",
	}
}

fn describe_registry_status(status: &RegistryStatus) -> &'static str {
	match status {
		RegistryStatus::Active => "Active",
		RegistryStatus::Revoked => "Revoked",
		RegistryStatus::Deleted => "Deleted",
	}
}

fn print_timeline(records: &[StateEventRecord], next_cursor: Option<u32>, full: bool) {
	println!("\n⏱️ Token Timeline");
	if records.is_empty() {
		println!("  ↳ • (no events)");
	} else {
		let limit = if full { records.len() } else { records.len().min(8) };
		for (idx, event) in records.iter().take(limit).enumerate() {
			let action_utf8 = String::from_utf8(event.action.clone())
				.unwrap_or_else(|_| format!("0x{}", hex::encode(&event.action)));
			let digest = format!("0x{}", hex::encode(event.digest));
			println!(
				"  ↳ • #{:<2} action={:<24} block=#{} extrinsic={} digest={}",
				idx + 1,
				action_utf8,
				event.seal.height,
				event.seal.index,
				digest
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
