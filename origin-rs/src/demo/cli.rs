use crate::demo::util::{RunMode, TxFlow, ViewStyle};
use anyhow::{anyhow, Result};
use std::{iter::Peekable, slice};

/// Common CLI options shared by all demos.
#[derive(Clone, Debug)]
pub struct CommonCliOptions {
	pub view: ViewStyle,
	pub output_json: bool,
	pub node: Option<String>,
	pub mode: RunMode,
	pub flow: TxFlow,
}

/// Result of parsing the common flags. The `rest` vector preserves
/// demo-specific arguments for a second parsing pass.
pub struct ParsedCommonArgs {
	pub common: CommonCliOptions,
	pub rest: Vec<String>,
}

pub fn parse_common_cli(args: &[String]) -> Result<ParsedCommonArgs> {
	let mut style: Option<ViewStyle> = None;
	let mut json = false;
	let mut node: Option<String> = None;
	let mut mode = RunMode::Transaction;
	let mut flow = TxFlow::Direct;
	let mut rest = Vec::new();
	let mut iter = args.iter().peekable();

	while let Some(arg) = iter.next() {
		match arg.as_str() {
			"--json" | "-j" => json = true,
			"--display" | "-d" => {
				let value = require_value(&mut iter, arg)?;
				style = Some(parse_display_style(&value)?);
			},
			_ if arg.starts_with("--display=") => {
				style = Some(parse_display_style(arg.trim_start_matches("--display="))?);
			},
			_ if arg.starts_with("-d=") => {
				style = Some(parse_display_style(arg.trim_start_matches("-d="))?);
			},
			"--node" | "-n" => node = Some(require_value(&mut iter, arg)?),
			_ if arg.starts_with("--node=") => {
				node = arg.splitn(2, '=').nth(1).map(|v| v.to_string());
			},
			_ if arg.starts_with("-n=") => {
				node = arg.splitn(2, '=').nth(1).map(|v| v.to_string());
			},
			"--mode" | "-m" => {
				let value = require_value(&mut iter, arg)?;
				mode = mode_from_value(&value)
					.ok_or_else(|| anyhow!("invalid --mode value: {value} (expected tx|view)"))?;
			},
			_ if arg.starts_with("--mode=") => {
				let value = arg.trim_start_matches("--mode=");
				mode = mode_from_value(value)
					.ok_or_else(|| anyhow!("invalid --mode value: {value} (expected tx|view)"))?;
			},
			"--flow" | "-f" => {
				let value = require_value(&mut iter, arg)?;
				flow = flow_from_value(&value).ok_or_else(|| {
					anyhow!("invalid --flow value: {value} (expected direct|relay)")
				})?;
			},
			_ if arg.starts_with("--flow=") => {
				let value = arg.trim_start_matches("--flow=");
				flow = flow_from_value(value).ok_or_else(|| {
					anyhow!("invalid --flow value: {value} (expected direct|relay)")
				})?;
			},
			"--" => {
				rest.extend(iter.map(|s| s.to_string()));
				break;
			},
			_ => rest.push(arg.clone()),
		}
	}

	let resolved_style =
		style.unwrap_or_else(|| if json { ViewStyle::Full } else { ViewStyle::Compact });
	Ok(ParsedCommonArgs {
		common: CommonCliOptions { view: resolved_style, output_json: json, node, mode, flow },
		rest,
	})
}

pub fn require_value<'a>(
	iter: &mut Peekable<slice::Iter<'a, String>>,
	flag: &str,
) -> Result<String> {
	iter.next()
		.map(|value| value.clone())
		.ok_or_else(|| anyhow!("{flag} expects a value"))
}

pub fn parse_display_style(value: &str) -> Result<ViewStyle> {
	style_from_value(value)
		.ok_or_else(|| anyhow!("invalid display style '{value}' (expected less|more/full)"))
}

pub fn style_from_value(value: impl AsRef<str>) -> Option<ViewStyle> {
	match value.as_ref().to_ascii_lowercase().as_str() {
		"full" | "more" => Some(ViewStyle::Full),
		"compact" | "less" => Some(ViewStyle::Compact),
		_ => None,
	}
}

pub fn mode_from_value(value: impl AsRef<str>) -> Option<RunMode> {
	match value.as_ref().to_ascii_lowercase().as_str() {
		"tx" | "transaction" => Some(RunMode::Transaction),
		"view" => Some(RunMode::View),
		_ => None,
	}
}

pub fn flow_from_value(value: impl AsRef<str>) -> Option<TxFlow> {
	match value.as_ref().to_ascii_lowercase().as_str() {
		"direct" | "signer" => Some(TxFlow::Direct),
		"relayed" | "relay" | "meta" => Some(TxFlow::Relayed),
		_ => None,
	}
}
