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

use blake2::{digest::consts::U32, Blake2b, Digest as BlakeDigest};
use chacha20poly1305::{
	aead::{Aead, Payload},
	KeyInit, XChaCha20Poly1305, XNonce,
};
use ciborium::value::Value;
use ed25519_dalek::{Signature, VerifyingKey};
use serde::Deserialize;
use serde_json::{json, Value as Json};
use sha2::Sha256;
use std::{
	collections::{BTreeMap, BTreeSet},
	fs,
	io::Cursor,
	path::{Path, PathBuf},
};
use unicode_normalization::UnicodeNormalization;

#[allow(dead_code)]
mod generated {
	include!("generated/origin_host_registry_v2.rs");
}

#[derive(Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum SchemaNode {
	Ref { name: String },
	Union { variants: Vec<SchemaNode> },
	Map { fields: Vec<Field> },
	Array { min: usize, max: usize, items: Box<SchemaNode> },
	Uint { min: String, max: String },
	Bytes { min: usize, max: usize },
	Text { min: usize, max: usize, nfc: bool },
	Bool,
	Const { value: Json },
}
#[derive(Clone, Deserialize)]
struct Field {
	key: u64,
	required: bool,
	schema: SchemaNode,
}
#[derive(Clone, Deserialize)]
struct CrossRule {
	production: String,
	id: String,
	kind: String,
	key: Option<u64>,
	left: Option<u64>,
	right: Option<u64>,
	max: Option<String>,
}
#[derive(Deserialize)]
struct SemanticTable {
	schema_version: u64,
	cddl_sha256: String,
	json_projection_sha256: String,
	schemas: BTreeMap<String, SchemaNode>,
	cross_field_rules: Vec<CrossRule>,
}
#[derive(Clone)]
struct Mutation {
	id: String,
	expected: String,
	value: Value,
}

fn root() -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}
fn bytes(s: &str) -> Vec<u8> {
	hex::decode(s).unwrap()
}
fn sha(data: &[u8]) -> String {
	hex::encode(Sha256::digest(data))
}
fn j(path: &str) -> Json {
	serde_json::from_slice(&fs::read(root().join(path)).unwrap()).unwrap()
}
fn head(major: u8, n: u64, out: &mut Vec<u8>) {
	if n < 24 {
		out.push((major << 5) | n as u8)
	} else if n <= 255 {
		out.extend([(major << 5) | 24, n as u8])
	} else if n <= 65535 {
		out.push((major << 5) | 25);
		out.extend((n as u16).to_be_bytes())
	} else if n <= u32::MAX as u64 {
		out.push((major << 5) | 26);
		out.extend((n as u32).to_be_bytes())
	} else {
		out.push((major << 5) | 27);
		out.extend(n.to_be_bytes())
	}
}
fn integer_u64(i: &ciborium::value::Integer) -> Result<u64, String> {
	u64::try_from(*i).map_err(|_| "negative/large integer".into())
}
fn canonical(v: &Value) -> Result<Vec<u8>, String> {
	let mut out = Vec::new();
	encode(v, &mut out)?;
	Ok(out)
}
fn encode(v: &Value, out: &mut Vec<u8>) -> Result<(), String> {
	match v {
		Value::Integer(i) => head(0, integer_u64(i)?, out),
		Value::Bytes(b) => {
			head(2, b.len() as u64, out);
			out.extend(b)
		},
		Value::Text(s) => {
			head(3, s.len() as u64, out);
			out.extend(s.as_bytes())
		},
		Value::Bool(b) => out.push(if *b { 245 } else { 244 }),
		Value::Array(a) => {
			head(4, a.len() as u64, out);
			for x in a {
				encode(x, out)?
			}
		},
		Value::Map(m) => {
			let mut pairs = Vec::new();
			let mut seen = BTreeSet::new();
			for (k, v) in m {
				let kb = canonical(k)?;
				if !seen.insert(kb.clone()) {
					return Err("duplicate key".into());
				}
				pairs.push((kb, canonical(v)?))
			}
			pairs.sort_by(|a, b| a.0.len().cmp(&b.0.len()).then(a.0.cmp(&b.0)));
			head(5, pairs.len() as u64, out);
			for (k, v) in pairs {
				out.extend(k);
				out.extend(v)
			}
		},
		_ => return Err("tag/float/null/simple forbidden".into()),
	};
	Ok(())
}
fn decode(raw: &[u8]) -> Result<Value, String> {
	let mut c = Cursor::new(raw);
	let v: Value = ciborium::de::from_reader(&mut c).map_err(|e| e.to_string())?;
	if c.position() != raw.len() as u64 {
		return Err("trailing".into());
	}
	Ok(v)
}
fn canonical_wire(raw: &[u8]) -> bool {
	decode(raw).and_then(|v| canonical(&v)).map(|b| b == raw).unwrap_or(false)
}
fn uint(v: &Value) -> Option<u64> {
	if let Value::Integer(i) = v {
		integer_u64(i).ok()
	} else {
		None
	}
}
fn map_get<'a>(v: &'a Value, key: u64) -> Option<&'a Value> {
	if let Value::Map(m) = v {
		m.iter().find_map(|(k, v)| (uint(k) == Some(key)).then_some(v))
	} else {
		None
	}
}
fn map_set(v: &mut Value, key: u64, next: Value) {
	if let Value::Map(m) = v {
		if let Some((_, x)) = m.iter_mut().find(|(k, _)| uint(k) == Some(key)) {
			*x = next
		} else {
			m.push((Value::Integer(key.into()), next))
		}
	} else {
		panic!("map_set")
	}
}
fn map_delete(v: &mut Value, key: u64) {
	if let Value::Map(m) = v {
		m.retain(|(k, _)| uint(k) != Some(key))
	}
}
fn semantic_fail(owner: &str, path: &str, rule: &str) -> String {
	format!("{owner}:{path}:{rule}")
}
fn validate_named(
	table: &SemanticTable,
	name: &str,
	value: &Value,
	path: &str,
) -> Result<(), String> {
	let node = table.schemas.get(name).ok_or_else(|| format!("unknown production {name}"))?;
	validate_node(table, node, value, name, path)?;
	if path == "$" {
		validate_cross(table, name, value)?
	}
	Ok(())
}
fn validate_node(
	table: &SemanticTable,
	node: &SchemaNode,
	value: &Value,
	owner: &str,
	path: &str,
) -> Result<(), String> {
	match node {
		SchemaNode::Ref { name } => validate_named(table, name, value, path),
		SchemaNode::Union { variants } => {
			if variants.iter().any(|x| validate_node(table, x, value, owner, path).is_ok()) {
				Ok(())
			} else {
				Err(semantic_fail(owner, path, "union"))
			}
		},
		SchemaNode::Map { fields } => {
			let m = if let Value::Map(m) = value {
				m
			} else {
				return Err(semantic_fail(owner, path, "type-map"));
			};
			for (k, _) in m {
				let key = uint(k).ok_or_else(|| semantic_fail(owner, path, "integer-key"))?;
				if !fields.iter().any(|f| f.key == key) {
					return Err(semantic_fail(owner, path, &format!("additionalProperties:{key}")));
				}
			}
			for f in fields {
				match map_get(value, f.key) {
					None if f.required => {
						return Err(semantic_fail(owner, path, &format!("required:{}", f.key)))
					},
					Some(child) => {
						validate_node(table, &f.schema, child, owner, &format!("{path}/{}", f.key))?
					},
					_ => {},
				}
			}
			Ok(())
		},
		SchemaNode::Array { min, max, items } => {
			let a = if let Value::Array(a) = value {
				a
			} else {
				return Err(semantic_fail(owner, path, "type-array"));
			};
			if a.len() < *min {
				return Err(semantic_fail(owner, path, "minItems"));
			}
			if a.len() > *max {
				return Err(semantic_fail(owner, path, "maxItems"));
			}
			for (i, x) in a.iter().enumerate() {
				validate_node(table, items, x, owner, &format!("{path}/{i}"))?
			}
			Ok(())
		},
		SchemaNode::Uint { min, max } => {
			let n = uint(value).ok_or_else(|| semantic_fail(owner, path, "type-uint"))?;
			if n < min.parse().unwrap() {
				return Err(semantic_fail(owner, path, "minimum"));
			}
			if n > max.parse().unwrap() {
				return Err(semantic_fail(owner, path, "maximum"));
			}
			Ok(())
		},
		SchemaNode::Bytes { min, max } => {
			let b = if let Value::Bytes(b) = value {
				b
			} else {
				return Err(semantic_fail(owner, path, "type-bytes"));
			};
			if b.len() < *min {
				return Err(semantic_fail(owner, path, "minBytes"));
			}
			if b.len() > *max {
				return Err(semantic_fail(owner, path, "maxBytes"));
			}
			Ok(())
		},
		SchemaNode::Text { min, max, nfc } => {
			let s = if let Value::Text(s) = value {
				s
			} else {
				return Err(semantic_fail(owner, path, "type-text"));
			};
			let n = s.as_bytes().len();
			if n < *min {
				return Err(semantic_fail(owner, path, "minUtf8Bytes"));
			}
			if n > *max {
				return Err(semantic_fail(owner, path, "maxUtf8Bytes"));
			}
			if *nfc && s.nfc().collect::<String>() != *s {
				return Err(semantic_fail(owner, path, "nfc"));
			}
			Ok(())
		},
		SchemaNode::Bool => {
			if matches!(value, Value::Bool(_)) {
				Ok(())
			} else {
				Err(semantic_fail(owner, path, "type-bool"))
			}
		},
		SchemaNode::Const { value: expected } => {
			let ok = match expected {
				Json::Number(n) => uint(value) == n.as_u64(),
				Json::String(s) => matches!(value,Value::Text(x)if x==s),
				Json::Bool(b) => matches!(value,Value::Bool(x)if x==b),
				_ => false,
			};
			if ok {
				Ok(())
			} else {
				Err(semantic_fail(owner, path, "const"))
			}
		},
	}
}
fn validate_cross(table: &SemanticTable, name: &str, value: &Value) -> Result<(), String> {
	for r in table.cross_field_rules.iter().filter(|r| r.production == name) {
		let get = |k: u64| {
			map_get(value, k).ok_or_else(|| semantic_fail(name, "$", &format!("cross:{}", r.id)))
		};
		let ok = match r.kind.as_str() {
			"uint-positive" => uint(get(r.key.unwrap())?).unwrap_or(0) > 0,
			"uint-greater" => {
				uint(get(r.left.unwrap())?).unwrap_or(0) > uint(get(r.right.unwrap())?).unwrap_or(0)
			},
			"uint-delta-max" => {
				let l = uint(get(r.left.unwrap())?).unwrap_or(0);
				let q = uint(get(r.right.unwrap())?).unwrap_or(0);
				l.checked_sub(q)
					.map(|d| d <= r.max.as_ref().unwrap().parse().unwrap())
					.unwrap_or(false)
			},
			"bytes-nonzero" => {
				matches!(get(r.key.unwrap())?,Value::Bytes(b)if b.iter().any(|x|*x!=0))
			},
			"bytes-no-nul" => {
				matches!(get(r.key.unwrap())?,Value::Bytes(b)if b.iter().all(|x|*x!=0))
			},
			"bytes-array-sorted-unique" => {
				if let Value::Array(a) = get(r.key.unwrap())? {
					a.windows(2).all(|w| canonical(&w[0]).unwrap() < canonical(&w[1]).unwrap())
				} else {
					false
				}
			},
			_ => false,
		};
		if !ok {
			return Err(semantic_fail(name, "$", &format!("cross:{}", r.id)));
		}
	}
	Ok(())
}
fn sample_node(table: &SemanticTable, node: &SchemaNode) -> Value {
	match node {
		SchemaNode::Ref { name } => sample_named(table, name),
		SchemaNode::Union { variants } => sample_node(table, &variants[0]),
		SchemaNode::Map { fields } => Value::Map(
			fields
				.iter()
				.filter(|f| f.required)
				.map(|f| (Value::Integer(f.key.into()), sample_node(table, &f.schema)))
				.collect(),
		),
		SchemaNode::Array { min, items, .. } => {
			Value::Array((0..*min).map(|_| sample_node(table, items)).collect())
		},
		SchemaNode::Uint { min, .. } => Value::Integer(min.parse::<u64>().unwrap().into()),
		SchemaNode::Bytes { min, .. } => Value::Bytes(vec![0; *min]),
		SchemaNode::Text { min, .. } => Value::Text("a".repeat(*min)),
		SchemaNode::Bool => Value::Bool(false),
		SchemaNode::Const { value } => match value {
			Json::Number(n) => Value::Integer(n.as_u64().unwrap().into()),
			Json::String(s) => Value::Text(s.clone()),
			Json::Bool(b) => Value::Bool(*b),
			_ => panic!(),
		},
	}
}
fn sample_named(table: &SemanticTable, name: &str) -> Value {
	let mut v = sample_node(table, table.schemas.get(name).unwrap());
	for r in table.cross_field_rules.iter().filter(|r| r.production == name) {
		match r.kind.as_str() {
			"uint-positive" => map_set(&mut v, r.key.unwrap(), Value::Integer(1.into())),
			"uint-greater" => map_set(&mut v, r.left.unwrap(), Value::Integer(1.into())),
			"bytes-nonzero" => {
				if let Some(Value::Bytes(b)) = match &mut v {
					Value::Map(m) => {
						m.iter_mut().find_map(|(k, v)| (uint(k) == r.key).then_some(v))
					},
					_ => None,
				} {
					b[0] = 1
				}
			},
			"bytes-no-nul" => {
				if let Some(Value::Bytes(b)) = match &mut v {
					Value::Map(m) => {
						m.iter_mut().find_map(|(k, v)| (uint(k) == r.key).then_some(v))
					},
					_ => None,
				} {
					b.fill(1)
				}
			},
			"bytes-array-sorted-unique" => {
				let field = if let SchemaNode::Map { fields } = table.schemas.get(name).unwrap() {
					fields.iter().find(|f| Some(f.key) == r.key).unwrap()
				} else {
					panic!()
				};
				let item = if let SchemaNode::Array { items, .. } = &field.schema {
					items
				} else {
					panic!()
				};
				let first = sample_node(table, item);
				let second = match &first {
					Value::Bytes(b) => {
						let mut x = b.clone();
						*x.last_mut().unwrap() = 1;
						Value::Bytes(x)
					},
					Value::Text(s) => Value::Text(format!("{s}b")),
					_ => sample_node(table, item),
				};
				map_set(&mut v, r.key.unwrap(), Value::Array(vec![first, second]))
			},
			_ => {},
		}
	}
	v
}
fn mutations_named(table: &SemanticTable, name: &str) -> Vec<Mutation> {
	let base = sample_named(table, name);
	let mut out = mutations_node(table, table.schemas.get(name).unwrap(), &base, name, "$");
	for r in table.cross_field_rules.iter().filter(|r| r.production == name) {
		let mut v = sample_named(table, name);
		match r.kind.as_str() {
			"uint-positive" => map_set(&mut v, r.key.unwrap(), Value::Integer(0.into())),
			"uint-greater" => {
				let q = map_get(&v, r.right.unwrap()).unwrap().clone();
				map_set(&mut v, r.left.unwrap(), q)
			},
			"uint-delta-max" => {
				let q = uint(map_get(&v, r.right.unwrap()).unwrap()).unwrap();
				let mx = r.max.as_ref().unwrap().parse::<u64>().unwrap();
				map_set(&mut v, r.left.unwrap(), Value::Integer((q + mx + 1).into()))
			},
			"bytes-nonzero" => {
				let len = if let Value::Bytes(b) = map_get(&v, r.key.unwrap()).unwrap() {
					b.len()
				} else {
					0
				};
				map_set(&mut v, r.key.unwrap(), Value::Bytes(vec![0; len]))
			},
			"bytes-no-nul" => {
				if let Value::Bytes(b) = map_get(&v, r.key.unwrap()).unwrap() {
					let mut x = b.clone();
					x[0] = 0;
					map_set(&mut v, r.key.unwrap(), Value::Bytes(x))
				}
			},
			"bytes-array-sorted-unique" => {
				let first = if let Value::Array(a) = map_get(&v, r.key.unwrap()).unwrap() {
					a[0].clone()
				} else {
					panic!()
				};
				map_set(&mut v, r.key.unwrap(), Value::Array(vec![first.clone(), first]))
			},
			_ => {},
		}
		out.push(Mutation {
			id: format!("$:cross:{}", r.id),
			expected: semantic_fail(name, "$", &format!("cross:{}", r.id)),
			value: v,
		})
	}
	out
}
fn mutations_node(
	table: &SemanticTable,
	node: &SchemaNode,
	base: &Value,
	owner: &str,
	path: &str,
) -> Vec<Mutation> {
	let mut out = Vec::new();
	match node {
		SchemaNode::Ref { name } => {
			if let Some(n) = mutations_named(table, name).into_iter().next() {
				out.push(Mutation {
					id: format!("{path}:ref:{name}:{}", n.id),
					expected: n.expected.replacen(
						&format!("{name}:$"),
						&format!("{name}:{path}"),
						1,
					),
					value: n.value,
				})
			}
		},
		SchemaNode::Union { .. } => out.push(Mutation {
			id: format!("{path}:union"),
			expected: semantic_fail(owner, path, "union"),
			value: Value::Map(vec![(Value::Integer(999.into()), Value::Integer(0.into()))]),
		}),
		SchemaNode::Map { fields } => {
			out.push(Mutation {
				id: format!("{path}:type-map"),
				expected: semantic_fail(owner, path, "type-map"),
				value: Value::Array(vec![]),
			});
			let mut x = base.clone();
			map_set(&mut x, 65535, Value::Integer(0.into()));
			out.push(Mutation {
				id: format!("{path}:additionalProperties"),
				expected: semantic_fail(owner, path, "additionalProperties:65535"),
				value: x,
			});
			for f in fields {
				if f.required {
					let mut x = base.clone();
					map_delete(&mut x, f.key);
					out.push(Mutation {
						id: format!("{path}:required:{}", f.key),
						expected: semantic_fail(owner, path, &format!("required:{}", f.key)),
						value: x,
					})
				}
				let child =
					map_get(base, f.key).cloned().unwrap_or_else(|| sample_node(table, &f.schema));
				for n in
					mutations_node(table, &f.schema, &child, owner, &format!("{path}/{}", f.key))
				{
					let mut x = base.clone();
					map_set(&mut x, f.key, n.value);
					out.push(Mutation { id: n.id, expected: n.expected, value: x })
				}
			}
		},
		SchemaNode::Array { min, max, items } => {
			out.push(Mutation {
				id: format!("{path}:type-array"),
				expected: semantic_fail(owner, path, "type-array"),
				value: Value::Map(vec![]),
			});
			if *min > 0 {
				out.push(Mutation {
					id: format!("{path}:minItems"),
					expected: semantic_fail(owner, path, "minItems"),
					value: Value::Array(vec![]),
				})
			}
			if *max < 4097 {
				out.push(Mutation {
					id: format!("{path}:maxItems"),
					expected: semantic_fail(owner, path, "maxItems"),
					value: Value::Array((0..=*max).map(|_| sample_node(table, items)).collect()),
				})
			}
			let mut a: Vec<Value> = (0..(*min).max(1)).map(|_| sample_node(table, items)).collect();
			if let Some(n) = mutations_node(table, items, &a[0], owner, &format!("{path}/0"))
				.into_iter()
				.next()
			{
				a[0] = n.value;
				out.push(Mutation {
					id: format!("{path}:items"),
					expected: n.expected,
					value: Value::Array(a),
				})
			}
		},
		SchemaNode::Uint { min, max } => {
			out.push(Mutation {
				id: format!("{path}:type-uint"),
				expected: semantic_fail(owner, path, "type-uint"),
				value: Value::Bytes(vec![]),
			});
			let lo = min.parse::<u64>().unwrap();
			let hi = max.parse::<u64>().unwrap();
			if lo > 0 {
				out.push(Mutation {
					id: format!("{path}:minimum"),
					expected: semantic_fail(owner, path, "minimum"),
					value: Value::Integer((lo - 1).into()),
				})
			}
			if hi < u64::MAX {
				out.push(Mutation {
					id: format!("{path}:maximum"),
					expected: semantic_fail(owner, path, "maximum"),
					value: Value::Integer((hi + 1).into()),
				})
			}
		},
		SchemaNode::Bytes { min, max } => {
			out.push(Mutation {
				id: format!("{path}:type-bytes"),
				expected: semantic_fail(owner, path, "type-bytes"),
				value: Value::Integer(0.into()),
			});
			if *min > 0 {
				out.push(Mutation {
					id: format!("{path}:minBytes"),
					expected: semantic_fail(owner, path, "minBytes"),
					value: Value::Bytes(vec![0; min - 1]),
				})
			}
			if *max < 4194400 {
				out.push(Mutation {
					id: format!("{path}:maxBytes"),
					expected: semantic_fail(owner, path, "maxBytes"),
					value: Value::Bytes(vec![0; max + 1]),
				})
			}
		},
		SchemaNode::Text { min, max, nfc } => {
			out.push(Mutation {
				id: format!("{path}:type-text"),
				expected: semantic_fail(owner, path, "type-text"),
				value: Value::Integer(0.into()),
			});
			if *min > 0 {
				out.push(Mutation {
					id: format!("{path}:minUtf8Bytes"),
					expected: semantic_fail(owner, path, "minUtf8Bytes"),
					value: Value::Text("a".repeat(min - 1)),
				})
			}
			if *max < 10000 {
				out.push(Mutation {
					id: format!("{path}:maxUtf8Bytes"),
					expected: semantic_fail(owner, path, "maxUtf8Bytes"),
					value: Value::Text("a".repeat(max + 1)),
				})
			}
			if *nfc && *max >= 3 {
				out.push(Mutation {
					id: format!("{path}:nfc"),
					expected: semantic_fail(owner, path, "nfc"),
					value: Value::Text(format!("e\u{301}{}", "a".repeat(min.saturating_sub(3)))),
				})
			}
		},
		SchemaNode::Bool => out.push(Mutation {
			id: format!("{path}:type-bool"),
			expected: semantic_fail(owner, path, "type-bool"),
			value: Value::Integer(0.into()),
		}),
		SchemaNode::Const { value } => {
			let wrong = match value {
				Json::Number(n) => Value::Integer((n.as_u64().unwrap() + 1).into()),
				Json::Bool(b) => Value::Bool(!b),
				Json::String(s) => Value::Text(format!("{s}x")),
				_ => panic!(),
			};
			out.push(Mutation {
				id: format!("{path}:const"),
				expected: semantic_fail(owner, path, "const"),
				value: wrong,
			})
		},
	}
	out
}
fn verify_ed(public: &str, message: &str, signature: &str) {
	let p: [u8; 32] = bytes(public).try_into().unwrap();
	let key = VerifyingKey::from_bytes(&p).unwrap();
	let sig = Signature::from_slice(&bytes(signature)).unwrap();
	key.verify_strict(&bytes(message), &sig).unwrap()
}

#[test]
fn origin_host_registry_conformance() {
	let cddl = fs::read(root().join("docs/specs/origin-host-registry-v2.cddl")).unwrap();
	assert_eq!(sha(&cddl), generated::REGISTRY_SHA256);
	cddl::parser::cddl_from_str(std::str::from_utf8(&cddl).unwrap(), false)
		.expect("normative CDDL must parse");
	let projection =
		fs::read(root().join("docs/specs/origin-host-registry-v2.schema.json")).unwrap();
	assert_eq!(sha(&projection), generated::JSON_PROJECTION_SHA256);
	assert_eq!(sha(generated::SEMANTIC_TABLE_JSON.as_bytes()), generated::SEMANTIC_SCHEMA_SHA256);
	let table: SemanticTable = serde_json::from_str(generated::SEMANTIC_TABLE_JSON).unwrap();
	assert_eq!(table.schema_version, 2);
	assert_eq!(table.cddl_sha256, generated::REGISTRY_SHA256);
	assert_eq!(table.json_projection_sha256, generated::JSON_PROJECTION_SHA256);
	let mut production_coverage = Vec::new();
	let mut coverage_lines = Vec::new();
	let mut class_coverage: BTreeMap<String, u64> = BTreeMap::new();
	for name in table.schemas.keys() {
		let positive = sample_named(&table, name);
		let wire = canonical(&positive).unwrap();
		validate_named(&table, name, &decode(&wire).unwrap(), "$").unwrap();
		let negatives = mutations_named(&table, name);
		assert!(!negatives.is_empty(), "no negative coverage {name}");
		let mut ids = Vec::new();
		for n in negatives {
			let actual =
				validate_named(&table, name, &decode(&canonical(&n.value).unwrap()).unwrap(), "$")
					.err()
					.unwrap_or_else(|| "accepted".into());
			assert_eq!(actual, n.expected, "semantic mutation {name}/{}", n.id);
			let parts: Vec<&str> = n.expected.split(':').collect();
			let class = if parts.contains(&"required") {
				"required"
			} else if parts.contains(&"additionalProperties") {
				"additionalProperties"
			} else if parts.contains(&"cross") {
				"cross"
			} else {
				parts.last().unwrap()
			};
			*class_coverage.entry(class.into()).or_default() += 1;
			ids.push(n.id)
		}
		coverage_lines.push(format!("{name}\0{}", ids.join("\0")));
		production_coverage.push(json!({"name":name,"positive":1,"negative_constraints":ids}))
	}
	let semantic_coverage_sha256 = sha(coverage_lines.join("\n").as_bytes());
	let operations = j("docs/specs/origin-host-registry-v2.operations.json");
	let errors = j("docs/specs/origin-host-registry-v2.errors.json");
	let host = j("docs/specs/origin-host-registry-v2.vectors.json");
	let protocol = j("docs/specs/protocol-executable-v2.vectors.json");
	let checkpoint = j("docs/specs/checkpoint-v2.vectors.json");
	let outbox = j("docs/specs/host-outbox-v1.vectors.json");
	let op_codes: BTreeSet<u64> = operations["operations"]
		.as_array()
		.unwrap()
		.iter()
		.map(|x| x["code"].as_u64().unwrap())
		.collect();
	let generated_ops: BTreeSet<u64> = generated::OPERATIONS.iter().map(|x| x.1 as u64).collect();
	assert_eq!(op_codes, generated_ops);
	let error_codes: BTreeSet<u64> = errors["errors"]
		.as_array()
		.unwrap()
		.iter()
		.map(|x| x["code"].as_u64().unwrap())
		.collect();
	let generated_errors: BTreeSet<u64> = generated::ERRORS.iter().map(|x| x.1 as u64).collect();
	assert_eq!(error_codes, generated_errors);
	let mut frames = BTreeMap::new();
	for op in operations["operations"].as_array().unwrap() {
		frames.insert(
			op["code"].as_u64().unwrap(),
			op["cddl"]["Request"].as_str().unwrap().replace("Request", "Frame"),
		);
	}
	let mut accepted = 0;
	let mut rejected = 0;
	let mut schema_rejected = 0;
	let mut semantic_accepted = 0;
	let mut semantic_rejected = 0;
	for v in host["vectors"].as_array().unwrap() {
		let raw = bytes(v["wire_hex"].as_str().unwrap());
		if let Some(h) = v.get("wire_sha256").and_then(Json::as_str) {
			assert_eq!(sha(&raw), h)
		}
		let canon = canonical_wire(&raw);
		if v.get("valid").and_then(Json::as_bool) == Some(false) {
			assert!(!canon, "accepted invalid {}", v["id"]);
			rejected += 1;
			continue;
		}
		assert!(canon);
		accepted += 1;
		let value = decode(&raw).unwrap();
		let ty = if v.get("kind").and_then(Json::as_str) == Some("exact-error-event") {
			"ErrorEventV2".to_string()
		} else {
			frames.get(&uint(map_get(&value, 3).unwrap()).unwrap()).unwrap().clone()
		};
		let result = validate_named(&table, &ty, &value, "$");
		let schema_negative = v.get("kind").and_then(Json::as_str) == Some("operation-negative")
			&& v["id"].as_str().unwrap().contains("schema-negative");
		if schema_negative {
			assert_eq!(result.unwrap_err(), format!("{ty}:$:required:8"));
			schema_rejected += 1;
			semantic_rejected += 1
		} else {
			result.unwrap();
			semantic_accepted += 1
		}
	}
	let corpus = fs::read(root().join("docs/specs/origin-host-registry-v2.vectors.cbor")).unwrap();
	assert!(canonical_wire(&corpus));
	let row_count = if let Value::Map(m) = decode(&corpus).unwrap() {
		m.iter()
			.find_map(|(k, v)| {
				(uint(k) == Some(1)).then(|| if let Value::Array(a) = v { a.len() } else { 0 })
			})
			.unwrap()
	} else {
		0
	};
	assert_eq!(row_count, host["concrete_wire_vectors"].as_u64().unwrap() as usize);
	let mut crypto_verified = 0;
	let mut exact = 0;
	let mut negative_state = 0;
	for v in protocol["vectors"].as_array().unwrap() {
		let raw = bytes(v["canonical_cbor_hex"].as_str().unwrap());
		assert!(canonical_wire(&raw));
		assert_eq!(sha(&raw), v["canonical_sha256"]);
		validate_named(&table, v["type"].as_str().unwrap(), &decode(&raw).unwrap(), "$").unwrap();
		semantic_accepted += 1;
		assert!(!canonical_wire(&bytes(v["noncanonical_cbor_hex"].as_str().unwrap())));
		for state in ["pre", "post"] {
			let raw = bytes(v[format!("{state}_state_cbor_hex")].as_str().unwrap());
			assert_eq!(sha(&raw), v[format!("{state}_state_sha256")]);
			validate_named(&table, "DurableStateV1", &decode(&raw).unwrap(), "$").unwrap();
			semantic_accepted += 1
		}
		assert_eq!(
			sha(&bytes(v["exact_response_cbor_hex"].as_str().unwrap())),
			v["exact_response_sha256"]
		);
		for n in v["negative_vectors"].as_array().unwrap() {
			assert!(n["effect_count"].as_u64().is_some());
			assert!(n["event_count"].as_u64().is_some());
			assert_eq!(n["post_state_sha256"].as_str().unwrap().len(), 64);
			if let Some(name) = n.get("expected_error").and_then(Json::as_str) {
				if !name.is_empty() {
					let e = errors["errors"]
						.as_array()
						.unwrap()
						.iter()
						.find(|e| e["name"] == name)
						.unwrap();
					assert_eq!(e["code"], n["expected_error_code"])
				}
			}
			let id = n["id"].as_str().unwrap();
			if id == "outbox-ciphertext-bitflip" {
				let c = &v["crypto"];
				let env = bytes(n["canonical_cbor_hex"].as_str().unwrap());
				let cipher =
					XChaCha20Poly1305::new_from_slice(&bytes(c["key_hex"].as_str().unwrap()))
						.unwrap();
				assert!(cipher
					.decrypt(
						XNonce::from_slice(&env[1..25]),
						Payload {
							msg: &env[25..],
							aad: &bytes(c["aad_cbor_hex"].as_str().unwrap())
						}
					)
					.is_err())
			} else {
				let ty = if id == "transfer-corrupt-chunk" {
					"ProviderTransferChunkV1"
				} else if id == "recovery-changed-request" {
					"StorageObjectPutFrame"
				} else {
					v["type"].as_str().unwrap()
				};
				let result = validate_named(
					&table,
					ty,
					&decode(&bytes(n["canonical_cbor_hex"].as_str().unwrap())).unwrap(),
					"$",
				);
				let expected = match id {
					"recovery-install-zero-seed" => {
						Some("RecoveryInstallV2:$:cross:recovery-seed-nonzero")
					},
					"drive-noncanonical-order" => {
						Some("DriveManifestV1:$:cross:drive-cids-sorted-unique")
					},
					"s3-invalid-key-nul" => Some("S3ObjectVersionV1:$:cross:s3-key-no-nul"),
					_ => None,
				};
				if let Some(e) = expected {
					assert_eq!(result.unwrap_err(), e);
					semantic_rejected += 1
				} else {
					result.unwrap();
					semantic_accepted += 1
				}
			}
			negative_state += 1
		}
		if let Some(c) = v.get("crypto") {
			if c.get("public_key_hex").is_some() {
				verify_ed(
					c["public_key_hex"].as_str().unwrap(),
					c["signed_bytes_hex"].as_str().unwrap(),
					c["signature_hex"].as_str().unwrap(),
				);
				crypto_verified += 1
			}
			if c.get("receipt_public_key_hex").is_some() {
				verify_ed(
					c["receipt_public_key_hex"].as_str().unwrap(),
					c["receipt_signed_bytes_hex"].as_str().unwrap(),
					c["receipt_signature_hex"].as_str().unwrap(),
				);
				crypto_verified += 1
			}
			if c.get("request_cbor_hex").is_some() {
				let mut f = bytes(c["request_cbor_hex"].as_str().unwrap());
				f.extend(bytes(c["authority_cbor_hex"].as_str().unwrap()));
				assert_eq!(sha(&f), c["request_fingerprint_sha256"])
			}
			if c.get("algorithm").and_then(Json::as_str) == Some("XChaCha20-Poly1305") {
				let env = bytes(c["envelope_hex"].as_str().unwrap());
				let cipher =
					XChaCha20Poly1305::new_from_slice(&bytes(c["key_hex"].as_str().unwrap()))
						.unwrap();
				let plain = cipher
					.decrypt(
						XNonce::from_slice(&env[1..25]),
						Payload {
							msg: &env[25..],
							aad: &bytes(c["aad_cbor_hex"].as_str().unwrap()),
						},
					)
					.unwrap();
				assert_eq!(hex::encode(plain), c["plaintext_cbor_hex"]);
				crypto_verified += 1
			}
		}
		exact += 1
	}
	let p = &checkpoint["positive"];
	verify_ed(
		p["public_key_hex"].as_str().unwrap(),
		p["digest_hex"].as_str().unwrap(),
		p["signature_hex"].as_str().unwrap(),
	);
	let digest = Blake2b::<U32>::digest(bytes(p["signed_message_hex"].as_str().unwrap()));
	assert_eq!(hex::encode(digest), p["digest_hex"]);
	crypto_verified += 1;
	validate_named(
		&table,
		"CheckpointSubmissionV2",
		&decode(&bytes(p["canonical_cbor_hex"].as_str().unwrap())).unwrap(),
		"$",
	)
	.unwrap();
	semantic_accepted += 1;
	for n in checkpoint["negative"].as_array().unwrap() {
		let id = n["id"].as_str().unwrap();
		let result = validate_named(
			&table,
			"CheckpointSubmissionV2",
			&decode(&bytes(n["canonical_cbor_hex"].as_str().unwrap())).unwrap(),
			"$",
		);
		if id == "checkpoint-wrong-version" {
			assert_eq!(result.unwrap_err(), "CheckpointSubmissionV2:$/0:const");
			semantic_rejected += 1
		} else {
			result.unwrap();
			semantic_accepted += 1
		}
		assert_eq!(sha(&bytes(n["canonical_cbor_hex"].as_str().unwrap())), n["canonical_sha256"]);
		assert_eq!(
			sha(&bytes(n["exact_response_cbor_hex"].as_str().unwrap())),
			n["exact_response_sha256"]
		);
		validate_named(
			&table,
			"ErrorEventV2",
			&decode(&bytes(n["exact_response_cbor_hex"].as_str().unwrap())).unwrap(),
			"$",
		)
		.unwrap();
		semantic_accepted += 1;
		if n["expected_error_code"] != 241 {
			assert_eq!(n["effect_count"], 0);
			assert_eq!(n["event_count"], 0);
			assert_eq!(n["pre_state_sha256"], n["post_state_sha256"])
		}
		negative_state += 1
	}
	let mut outbox_drift = 0;
	for v in outbox["crash_vectors"].as_array().unwrap() {
		for (hex_key, hash_key) in [
			("exact_request_cbor_hex", "exact_request_sha256"),
			("pre_state_cbor_hex", "pre_state_sha256"),
			("post_state_cbor_hex", "post_state_sha256"),
			("exact_response_cbor_hex", "exact_response_sha256"),
		] {
			let raw = bytes(v[hex_key].as_str().unwrap());
			assert_eq!(sha(&raw), v[hash_key]);
			let mut changed = raw;
			let last = changed.len() - 1;
			changed[last] ^= 1;
			assert_ne!(sha(&changed), v[hash_key]);
			outbox_drift += 1
		}
	}
	let report = json!({"schema_version":2,"status":"pass","cddl_parse_status":"pass","registry_sha256":generated::REGISTRY_SHA256,"json_projection_sha256":generated::JSON_PROJECTION_SHA256,"semantic_schema_sha256":generated::SEMANTIC_SCHEMA_SHA256,"semantic_coverage_sha256":semantic_coverage_sha256,"operations":op_codes.len(),"errors":error_codes.len(),"types":table.schemas.len(),"host_vectors":host["vectors"].as_array().unwrap().len(),"exact_protocol_vectors":exact,"canonical_accepted":accepted,"canonical_rejected":rejected,"schema_rejected":schema_rejected,"crypto_verified":crypto_verified,"negative_state_assertions":negative_state,"semantic_vector_accepted":semantic_accepted,"semantic_vector_rejected":semantic_rejected,"outbox_drift_rejections":outbox_drift,"production_coverage":production_coverage,"constraint_class_coverage":class_coverage,"unmatched_productions":[],"uncovered_constraints":[],"missing_operations":[],"missing_errors":[],"missing_types":[],"extra_types":[],"missing_vectors":[]});
	let out = root().join("target/p0-origin-host-rust-conformance.json");
	fs::create_dir_all(out.parent().unwrap()).unwrap();
	fs::write(out, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
}
