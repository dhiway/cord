// Copyright 2019-2021 Dhiway.
// This file is part of CORD Platform.

//! CORD CLI Library.

#![warn(missing_docs)]

mod chain_spec;
#[macro_use]
mod service;
mod cli;
mod command;
mod rpc;

fn main() -> sc_cli::Result<()> {
	command::run()
}