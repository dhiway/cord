/*
 * This file is part of the CORD
 * Copyright (C) 2020  Dhiway
 *
 */

use wasm_builder_runner::WasmBuilder;

fn main() {
	WasmBuilder::new()
		.with_current_project()
		.with_wasm_builder_from_crates("2.0.0")
		.export_heap_base()
		.import_memory()
		.build()
}
