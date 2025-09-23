use sexpfmt::*;

use std::error::Error;
use std::io::Write;

fn main_inner() -> Result<()> {
	let mut reader = FormReader::new(std::io::stdin())?;
	while let Some((s, position)) = reader.get()? {
		let output = parse_form(s, position)?;
		print_sexp(output);
		println!();
		std::io::stdout().flush()?;
	}
	Ok(())
}

fn main() {
	match main_inner() {
		Ok(()) => {}
		Err(e) => {
			eprintln!("ERROR: {e}");

			// Print additional context if available
			let mut source = e.source();
			while let Some(err) = source {
				eprintln!("  Caused by: {err}");
				source = err.source();
			}

			std::process::exit(1);
		}
	}
}
