# `sexpfmt`

S-expressions are easy for machines to write, but generating formatted S-expressions can be painful.
`sexpfmt` formats an input stream in a consistent way such that the output is both line-diffable and human-readable.

The formatting style used by sexpfmt is highly regular, unlike what many Lispers and Schemers prefer. Each indentation 
increments spaces by a fixed number of spaces (by default, 2).

```sexp
(object 
  (object
    (name "croissant")
    (quantity 2))
  (object
    (name "latte")
    (quantity 1)
    (size "tall")))
```

The S-expression data format used is highly simplified compared to LISP's.
There is no support for quote, quasiquote, unquote, or dot pair-builders.
The character literal `#\ ` (for space) is not supported either. Use `#\space` instead.
There is also no support for `#1234 = ...` expressions to construct graphs.

---

## Setup and Installation

- To build and install this tool, you will need `Cargo` and a Rust toolchain.
- Navigate to the root of this repository with a shell, then run:

  ```
  cargo install --path .
  ```
- To run tests, you will also need `bash` and `Python3`

---

## Example Usage

```bash
$ cat my-file.sexp | sexpfmt > my-formatted-file.sexp
$ ./build/my-sexp-generator-program arg1 arg2 | sexpfmt >> formatted-logfile.sexp
```

For examples of `sexpfmt`'s behavior, see the `test` directory.

---

## TODO
- allow command line options to specify...
  - whether to print help and exit (e.g. `-h` or `--help`)
  - whether to normalize bookend tokens
  - the margin width and indent width.
- preserve comments when parsing.
- better error reporting if mis-formatted file is fed as input.
  - e.g. we have almost everything needed to provide line numbers.
- consider whether to support more features like quote, quasiquote, unquote, pair building, etc.
- allow file input, directly map file using OS API to handle very large files.
