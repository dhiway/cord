use codec::Decode;
use scale_info::TypeDef;
use subxt::Metadata;
use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
	let path = "origin-rs/metadata/origin-hub.scale";
	let bytes = fs::read(path)?;
	let mut cursor: &[u8] = &bytes;
	let metadata = Metadata::decode(&mut cursor)?;
	let entity = metadata
		.pallets()
		.find(|p| p.name() == "Entity")
		.expect("pallet Entity");
	for vf in entity.view_functions() {
		if vf.name() == "overview" {
			println!("Entity::overview");
			println!("  query_id: {:?}", vf.query_id());
			println!("  inputs:");
			for input in vf.inputs() {
				println!("    {} -> type id {}", input.name, input.ty);
			}
			println!("  output type id: {}", vf.output_ty());
			dump_type(&metadata, vf.output_ty(), 0);
		}
	}
	Ok(())
}

fn dump_type(metadata: &Metadata, ty: u32, indent: usize) {
	let t = metadata.types().resolve(ty).expect("type");
	let pad = "  ".repeat(indent);
	println!("{pad}- id {ty} kind {:?}", t.type_def);
	match &t.type_def {
		TypeDef::Composite(comp) => {
			for field in comp.fields() {
				let name = field.name().map_or("<unnamed>", |v| v);
				println!("{pad}  field {name}: {}", field.ty().id);
				dump_type(metadata, field.ty().id, indent + 2);
			}
		},
		TypeDef::Variant(var) => {
			for v in var.variants() {
				println!("{pad}  variant {} (index {}):", v.name(), v.index());
				for f in v.fields() {
					let name = f.name().map_or("<unnamed>", |v| v);
					println!("{pad}    field {name}: {}", f.ty().id);
					dump_type(metadata, f.ty().id, indent + 3);
				}
			}
		},
		TypeDef::Sequence(seq) => {
			println!("{pad}  seq element: {}", seq.type_param().id());
			dump_type(metadata, seq.type_param().id(), indent + 1);
		},
		TypeDef::Array(arr) => {
			println!("{pad}  array len {} elem {}", arr.len(), arr.type_param().id());
			dump_type(metadata, arr.type_param().id(), indent + 1);
		},
		TypeDef::Tuple(tup) => {
			for (idx, id) in tup.fields().iter().enumerate() {
				println!("{pad}  tuple[{idx}]: {}", id.id());
				dump_type(metadata, id.id(), indent + 1);
			}
		},
		_ => {},
	}
}
