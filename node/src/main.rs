#![warn(missing_docs)]

mod chain_spec;
#[macro_use]
mod service;
mod cli;
mod rpc;
mod command;

// #[macro_use]
// extern crate hex_literal;

fn main() -> sc_cli::Result<()> {
	command::run()
}
