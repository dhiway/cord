/*
 * This file is part of the CORD
 * Copyright (C) 2020  Dhiway
 *
 */

mod chain_spec;
#[macro_use]
mod service;
mod cli;
mod rpc;
mod command;

fn main() -> sc_cli::Result<()> {
	command::run()
}
