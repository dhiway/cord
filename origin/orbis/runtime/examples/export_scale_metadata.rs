// This file is part of CORD – https://cord.network

// Copyright (C) Dhiway Networks Pvt. Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later

// CORD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// CORD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with CORD. If not, see <https://www.gnu.org/licenses/>.

//! Export complete Commons SCALE metadata and a deterministic logical-type registry.

use std::{env, fs, path::PathBuf};

use codec::Encode;
use frame_metadata::RuntimeMetadata;
use origin_commons_runtime::Runtime;
use scale_info::{form::PortableForm, PortableRegistry, Type, TypeDef, TypeDefPrimitive};
use serde_json::{json, Value};
use sp_crypto_hashing::sha2_256;

const LOGICAL_TYPES: &[(&str, &[&str])] = &[
	("cord::storage::MmrLeafV1", &["data_root", "data_size", "total_size"]),
	("cord::storage::MmrProofV1", &["peaks", "leaf", "leaf_proof"]),
	("cord::storage::CommitmentV1", &["mmr_root", "start_seq", "leaf_count"]),
	("cord::storage::ChunkLocationV1", &["leaf_index", "chunk_index"]),
	("cord::storage::CommitmentPayloadV2", &["version", "bucket_id", "commitment", "nonce"]),
	(
		"cord::storage::CheckpointFallbackPromotionV1",
		&["version", "bucket_id", "snapshot_nonce", "duty_id"],
	),
];

fn path_of(ty: &Type<PortableForm>) -> String {
	ty.path.segments.iter().map(String::as_str).collect::<Vec<_>>().join("::")
}

fn expected_shape(path: &str) -> Option<Value> {
	match path {
		"cord::storage::MmrLeafV1" => Some(json!({
			"composite": [["data_root", "H256"], ["data_size", "u64"], ["total_size", "u64"]]
		})),
		"cord::storage::MmrProofV1" => Some(json!({
			"composite": [
				["peaks", "Vec<H256>"],
				["leaf", "cord::storage::MmrLeafV1"],
				["leaf_proof", "Vec<H256>"]
			]
		})),
		"cord::storage::CommitmentV1" => Some(json!({
			"composite": [["mmr_root", "H256"], ["start_seq", "u64"], ["leaf_count", "u64"]]
		})),
		"cord::storage::ChunkLocationV1" => Some(json!({
			"composite": [["leaf_index", "u64"], ["chunk_index", "u32"]]
		})),
		"cord::storage::CommitmentPayloadV2" => Some(json!({
			"composite": [
				["version", "u8"],
				["bucket_id", "BucketId"],
				["commitment", "cord::storage::CommitmentV1"],
				["nonce", "u32"]
			]
		})),
		"cord::storage::CheckpointFallbackPromotionV1" => Some(json!({
			"composite": [
				["version", "u8"],
				["bucket_id", "BucketId"],
				["snapshot_nonce", "u32"],
				["duty_id", "H256"]
			]
		})),
		_ => None,
	}
}

fn validate_field_type(
	registry: &PortableRegistry,
	type_id: u32,
	expected: &str,
) -> Result<(), String> {
	let ty = registry
		.resolve(type_id)
		.ok_or_else(|| format!("portable type {type_id} is missing"))?;
	let path = path_of(ty);
	let matches = match expected {
		"H256" | "BucketId" => path.ends_with("::H256"),
		"u8" | "u32" | "u64" => matches!(
			&ty.type_def,
			TypeDef::Primitive(value) if primitive_name(value.clone()) == expected
		),
		"Vec<H256>" => matches!(
			&ty.type_def,
			TypeDef::Sequence(sequence) if registry
				.resolve(sequence.type_param.id)
				.is_some_and(|inner| path_of(inner).ends_with("::H256"))
		),
		logical => path == logical,
	};
	if matches {
		Ok(())
	} else {
		Err(format!("portable type {type_id} drift: expected {expected}, got path {path}"))
	}
}

fn validate_logical_type(
	registry: &PortableRegistry,
	path: &str,
	ty: &Type<PortableForm>,
) -> Result<(), String> {
	let expected_fields = LOGICAL_TYPES
		.iter()
		.find_map(|(candidate, fields)| (*candidate == path).then_some(*fields))
		.ok_or_else(|| format!("unknown logical type {path}"))?;
	let TypeDef::Composite(composite) = &ty.type_def else {
		return Err(format!("{path} is not a composite SCALE type"))
	};
	let actual_fields = composite
		.fields
		.iter()
		.map(|field| field.name.as_deref().unwrap_or(""))
		.collect::<Vec<_>>();
	if actual_fields != expected_fields {
		return Err(format!(
			"{path} field drift: expected {expected_fields:?}, got {actual_fields:?}"
		))
	}
	let expected = expected_shape(path).ok_or_else(|| format!("missing shape for {path}"))?;
	let expected_types = expected["composite"]
		.as_array()
		.ok_or_else(|| format!("invalid shape for {path}"))?;
	for (field, expected_field) in composite.fields.iter().zip(expected_types) {
		let expected_type = expected_field[1]
			.as_str()
			.ok_or_else(|| format!("invalid field shape for {path}"))?;
		validate_field_type(registry, field.ty.id, expected_type)?;
	}
	Ok(())
}

fn primitive_name(value: TypeDefPrimitive) -> &'static str {
	match value {
		TypeDefPrimitive::Bool => "bool",
		TypeDefPrimitive::Char => "char",
		TypeDefPrimitive::Str => "str",
		TypeDefPrimitive::U8 => "u8",
		TypeDefPrimitive::U16 => "u16",
		TypeDefPrimitive::U32 => "u32",
		TypeDefPrimitive::U64 => "u64",
		TypeDefPrimitive::U128 => "u128",
		TypeDefPrimitive::U256 => "u256",
		TypeDefPrimitive::I8 => "i8",
		TypeDefPrimitive::I16 => "i16",
		TypeDefPrimitive::I32 => "i32",
		TypeDefPrimitive::I64 => "i64",
		TypeDefPrimitive::I128 => "i128",
		TypeDefPrimitive::I256 => "i256",
	}
}

fn portable_definition(ty: &Type<PortableForm>) -> Value {
	match &ty.type_def {
		TypeDef::Composite(value) => json!({
			"kind": "composite",
			"fields": value.fields.iter().map(|field| json!({
				"name": field.name.as_deref(),
				"type_id": field.ty.id,
			})).collect::<Vec<_>>(),
		}),
		TypeDef::Variant(value) => json!({
			"kind": "variant",
			"variants": value.variants.iter().map(|variant| json!({
				"index": variant.index,
				"name": variant.name.as_str(),
				"fields": variant.fields.iter().map(|field| json!({
					"name": field.name.as_deref(),
					"type_id": field.ty.id,
				})).collect::<Vec<_>>(),
			})).collect::<Vec<_>>(),
		}),
		TypeDef::Sequence(value) => json!({"kind": "sequence", "type_id": value.type_param.id}),
		TypeDef::Array(value) => {
			json!({"kind": "array", "length": value.len, "type_id": value.type_param.id})
		},
		TypeDef::Tuple(value) => json!({
			"kind": "tuple",
			"type_ids": value.fields.iter().map(|field| field.id).collect::<Vec<_>>(),
		}),
		TypeDef::Primitive(value) =>
			json!({"kind": "primitive", "name": primitive_name(value.clone())}),
		TypeDef::Compact(value) => json!({"kind": "compact", "type_id": value.type_param.id}),
		TypeDef::BitSequence(value) => json!({
			"kind": "bit_sequence",
			"store_type_id": value.bit_store_type.id,
			"order_type_id": value.bit_order_type.id,
		}),
	}
}

fn export_registry(registry: &PortableRegistry, metadata_hash: &str) -> Result<Value, String> {
	let mut logical_count = 0usize;
	let mut rows = Vec::with_capacity(registry.types.len());
	for portable in &registry.types {
		let path = path_of(&portable.ty);
		let shape = expected_shape(&path);
		if shape.is_some() {
			validate_logical_type(registry, &path, &portable.ty)?;
			logical_count += 1;
		}
		rows.push(json!({
			"id": portable.id,
			"path": path,
			"shape": shape,
			"definition": portable_definition(&portable.ty),
		}));
	}
	if logical_count != LOGICAL_TYPES.len() {
		let candidates = registry
			.types
			.iter()
			.map(|portable| path_of(&portable.ty))
			.filter(|path| {
				LOGICAL_TYPES.iter().any(|(logical, _)| {
					path.ends_with(logical.rsplit("::").next().unwrap_or_default())
				})
			})
			.collect::<Vec<_>>();
		return Err(format!(
			"runtime metadata contains {logical_count}/{} required logical types; candidates: {:?}",
			LOGICAL_TYPES.len(),
			candidates,
		))
	}
	Ok(json!({
		"schema_version": 1,
		"runtime": "origin-commons-runtime",
		"metadata_sha256": metadata_hash,
		"type_count": rows.len(),
		"types": rows,
	}))
}

fn run() -> Result<(), String> {
	let mut args = env::args_os().skip(1);
	let metadata_path = PathBuf::from(args.next().ok_or("missing metadata SCALE output path")?);
	let registry_path = PathBuf::from(args.next().ok_or("missing portable registry output path")?);
	if args.next().is_some() {
		return Err("expected exactly two output paths".into())
	}

	// Some FRAME constants (for example ParachainInfo::ParachainId) are storage-backed. Build
	// metadata inside an empty deterministic externalities environment so exporting never depends
	// on a live node or database.
	let metadata = sp_io::TestExternalities::default().execute_with(Runtime::metadata);
	let encoded = metadata.encode();
	let metadata_hash = hex::encode(sha2_256(&encoded));
	let registry = match &metadata.1 {
		RuntimeMetadata::V14(value) => &value.types,
		RuntimeMetadata::V15(value) => &value.types,
		RuntimeMetadata::V16(value) => &value.types,
		other => return Err(format!("unsupported runtime metadata version {}", other.version())),
	};
	let registry_json = export_registry(registry, &metadata_hash)?;

	if let Some(parent) = metadata_path.parent() {
		fs::create_dir_all(parent).map_err(|error| error.to_string())?;
	}
	if let Some(parent) = registry_path.parent() {
		fs::create_dir_all(parent).map_err(|error| error.to_string())?;
	}
	fs::write(metadata_path, encoded).map_err(|error| error.to_string())?;
	fs::write(
		registry_path,
		serde_json::to_vec_pretty(&registry_json).map_err(|error| error.to_string())?,
	)
	.map_err(|error| error.to_string())?;
	Ok(())
}

fn main() {
	if let Err(error) = run() {
		eprintln!("BLOCKED Commons metadata export: {error}");
		std::process::exit(1);
	}
}
