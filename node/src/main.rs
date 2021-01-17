/*
 * This file is part of the CORD
 * Copyright (C) 2020-21  Dhiway
 *
 */

mod chain_spec;
#[macro_use]
mod service;
mod cli;
mod command;
mod rpc;

fn main() -> sc_cli::Result<()> {
	command::run()
}
