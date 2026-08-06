use clap::Parser;
use sexpfmt::{Config, SExpBookendStyle, SexpfmtError};

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Format S-expressions.
///
/// Reads S-expressions from stdin (or from FILES), formats them, and writes
/// them to stdout, one top-level S-expression at a time.
#[derive(Parser)]
#[command(version)]
struct Cli {
	/// Input files, formatted to stdout in order; reads stdin if none are
	/// given. Pass `-` to read stdin explicitly.
	files: Vec<PathBuf>,

	/// Number of spaces per indentation level.
	#[arg(long, default_value_t = 2)]
	indent: usize,

	/// Target maximum line width.
	#[arg(long, default_value_t = 80)]
	margin: usize,

	/// Normalize all list bookends to the given style instead of preserving
	/// each list's input style.
	#[arg(long, value_enum, value_name = "STYLE")]
	bookends: Option<Bookends>,

	/// Preserve `;` line comments instead of discarding them.
	#[arg(long)]
	preserve_comments: bool,

	/// In multi-line lists, keep a `:label` atom on the same line as the
	/// element that follows it.
	#[arg(long)]
	pair_labels: bool,
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum Bookends {
	/// ( )
	Parens,
	/// [ ]
	Square,
	/// { }
	Curly,
}

impl From<Bookends> for SExpBookendStyle {
	fn from(value: Bookends) -> Self {
		match value {
			Bookends::Parens => SExpBookendStyle::Parentheses,
			Bookends::Square => SExpBookendStyle::SquareBrackets,
			Bookends::Curly => SExpBookendStyle::CurlyBraces,
		}
	}
}

fn main() -> ExitCode {
	let cli = Cli::parse();
	let mut config = Config::default();
	config.parser.preserve_comments = cli.preserve_comments;
	config.printer.indent_width = cli.indent;
	config.printer.margin_width = cli.margin;
	config.printer.bookends = cli.bookends.map(SExpBookendStyle::from);
	config.printer.pair_labels = cli.pair_labels;

	let stdout = io::stdout().lock();
	let mut out = io::BufWriter::new(stdout);

	let inputs = if cli.files.is_empty() {
		vec![PathBuf::from("-")]
	} else {
		cli.files
	};
	for path in &inputs {
		let result = if path.as_os_str() == "-" {
			sexpfmt::format(io::stdin().lock(), &mut out, &config)
		} else {
			File::open(path)
				.map_err(SexpfmtError::from)
				.and_then(|file| sexpfmt::format(file, &mut out, &config))
		};
		match result {
			Ok(()) => {}
			// The reader of our output went away (e.g. `sexpfmt log.sexp | head`);
			// that is normal stream-consumer behavior, not an error.
			Err(e) if is_broken_pipe(&e) => return ExitCode::SUCCESS,
			Err(e) => {
				report_error(path, &e);
				return ExitCode::FAILURE;
			}
		}
	}
	ExitCode::SUCCESS
}

fn is_broken_pipe(e: &SexpfmtError) -> bool {
	matches!(e, SexpfmtError::Io { source } if source.kind() == io::ErrorKind::BrokenPipe)
}

fn report_error(path: &Path, e: &SexpfmtError) {
	// Every `SexpfmtError` variant renders its full context (position, source)
	// in its `Display` output, so no "caused by" chain is needed.
	if path.as_os_str() == "-" {
		eprintln!("ERROR: {e}");
	} else {
		eprintln!("ERROR: {}: {e}", path.display());
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_cli_definition() {
		use clap::CommandFactory;
		Cli::command().debug_assert();
	}
}
