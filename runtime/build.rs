/*
 * This file is part of the CORD
 * Copyright (C) 2020  Dhiway
 *
 */

use wasm_builder::WasmBuilder;

fn main() {
	WasmBuilder::new()
		.with_current_project()
		.export_heap_base()
		.import_memory()
		.build()
}
