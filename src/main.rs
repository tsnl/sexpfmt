mod sexp;
mod parser;
mod printer;

use sexp::*;
use parser::*;
use printer::*;

use std::io::{stdin, Read};

fn main() {
  let content = {
    let mut buf = Vec::with_capacity(8192);
    stdin().read_to_end(&mut buf).unwrap();
    String::from_utf8(buf).unwrap()
  };
  let output = parse_file(content);
  if !output.is_empty() {
    for s in output.into_iter() {
      print_sexp(s);
      println!();
    }
  }
}

