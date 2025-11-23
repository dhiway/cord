use std::fs;

use codec::Decode;
use scale_value::scale;
use subxt::Metadata;

fn main() -> Result<(), Box<dyn std::error::Error>> {
	// args: <metadata.scale> <comma-separated-bytes> [type_id]
	let mut args = std::env::args().skip(1);
	let meta_path = args
		.next()
		.expect("usage: decode_bytes <metadata.scale> <comma-separated-bytes> [type_id]");
	let bytes_arg = args.next().expect("missing bytes arg (comma-separated decimals)");
	let ty_id: u32 = args.next().map(|s| s.parse().expect("type id")).unwrap_or(551);

	let raw_bytes: Vec<u8> = bytes_arg
		.split(',')
		.filter(|s| !s.is_empty())
		.map(|s| s.trim().parse::<u8>().expect("byte"))
		.collect();

	let meta_bytes = fs::read(meta_path)?;
	let metadata = Metadata::decode(&mut &meta_bytes[..])?;

	let mut cursor: &[u8] = &raw_bytes;
	let val = scale::decode_as_type(&mut cursor, ty_id, metadata.types())
		.map_err(|e| format!("decode error: {e}"))?;

	println!("Decoded value:\n{:#?}", val);
	if !cursor.is_empty() {
		println!("{} trailing bytes: {:?}", cursor.len(), cursor);
	}
	Ok(())
}
