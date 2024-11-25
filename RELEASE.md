# Release Notes

## 0.2.0

- Streaming: forms are now formatted and emitted one-at-a-time.
  - This is a breaking change: old behavior was like `sponge` on the whole file. 
    Now, we only `sponge` one top-level form at a time.
  - Future releases may `sponge` even less: this behavior is unspecified going forward.
- More explicit I/O error handling.

## 0.1.0

- Basic CLI tool: read input as UTF-8 from `stdin` until EOF, format, then print to `stdout`.
