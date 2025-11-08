use clap::ValueEnum;
use color_eyre::eyre::WrapErr;
use std::{fmt, fs, path::PathBuf};

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
pub enum DocTopic {
	Identifiers,
	Tokens,
	Entities,
	Registers,
	Packets,
}

impl DocTopic {
	fn file_name(self) -> &'static str {
		match self {
			DocTopic::Identifiers => "identifiers.md",
			DocTopic::Tokens => "tokens.md",
			DocTopic::Entities => "entities.md",
			DocTopic::Registers => "registers.md",
			DocTopic::Packets => "packets.md",
		}
	}
}

impl fmt::Display for DocTopic {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(
			f,
			"{}",
			match self {
				DocTopic::Identifiers => "Identifiers",
				DocTopic::Tokens => "Tokens",
				DocTopic::Entities => "Entities",
				DocTopic::Registers => "Registers",
				DocTopic::Packets => "Packets",
			}
		)
	}
}

pub fn print(topic: DocTopic) -> color_eyre::Result<()> {
	let path = doc_path(topic);
	let body =
		fs::read_to_string(&path).wrap_err_with(|| format!("failed to read {}", path.display()))?;
	println!("\n======= {} =======\n", topic);
	println!("{}", body);
	Ok(())
}

fn doc_path(topic: DocTopic) -> PathBuf {
	let mut ancestors = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).ancestors();
	ancestors.next(); // manifest dir
	let workspace_root = ancestors
		.nth(1)
		.unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")));
	workspace_root
		.to_path_buf()
		.join("docs/examples/subxt-anchor")
		.join(topic.file_name())
}
