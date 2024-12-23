mod parser;
mod printer;
mod reader;
mod sexp;

use std::io::Write;

use parser::*;
use printer::*;
use reader::*;
use sexp::*;

fn main_inner() -> std::result::Result<(), Box<dyn std::error::Error>> {
	let mut reader = FormReader::new(std::io::stdin())?;
	loop {
		match reader.get()? {
			Some(s) => {
				let output = parse_form(s);
				print_sexp(output);
				println!();
				std::io::stdout().flush()?;
			}
			None => {
				break;
			}
		}
	}
	Ok(())
}

fn main() {
	match main_inner() {
		Ok(()) => {}
		Err(e) => {
			eprintln!("ERROR: {e}");
			std::process::exit(1);
		}
	}
}
