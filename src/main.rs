use std::io::{stdin, Read};

fn main() {
  let content = {
    let mut buf = Vec::with_capacity(8192);
    stdin().read_to_end(&mut buf).unwrap();
    String::from_utf8(buf).unwrap()
  };
  print!("{}", content)
}
