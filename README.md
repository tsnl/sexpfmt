# `sfmt`

S-expressions are an invaluable text-based data format for a number of applications, including expect testing.
`sfmt` formats an input stream in a consistent way such that the output is both machine-diffable and human-readable.

The formatting style used by sfmt is highly regular, unlike what many Lispers prefer. Each indentation increments spaces
by a fixed number of spaces (usually 2).

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

TODO:
- allow command line options to specify...
  - whether to normalize bookend tokens
  - the margin width
- preserve comments
- consider whether to support more features like quote, quasiquote, unquote, pair building, etc.
- allow file input, directly map file using OS API to handle very large files.
