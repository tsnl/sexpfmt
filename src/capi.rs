//! A minimal C API for embedding sexpfmt in other languages, e.g. to pipe
//! expect-test output through the formatter without shelling out.
//!
//! The C declarations live in `include/sexpfmt.h`. Build the C library with:
//!
//! ```sh
//! cargo rustc --release --features capi --lib --crate-type cdylib    # libsexpfmt.so / .dylib / .dll
//! cargo rustc --release --features capi --lib --crate-type staticlib # libsexpfmt.a
//! ```
//!
//! All strings returned by this API (formatted output and error messages) are
//! allocated with `malloc` and must be released with [`sexpfmt_str_free`].

use crate::{Config, SExpBookendStyle};
use std::ffi::{c_char, c_int};
use std::ptr;

/// Success.
pub const SEXPFMT_OK: c_int = 0;
/// The input could not be parsed or formatted; details in `*error`.
pub const SEXPFMT_ERR_FORMAT: c_int = 1;
/// An argument was invalid (null `input`/`out`, or a bad config value).
pub const SEXPFMT_ERR_INVALID_ARGUMENT: c_int = 2;
/// Allocating the result failed.
pub const SEXPFMT_ERR_NOMEM: c_int = 3;

/// Preserve each list's input bookend style ([`SexpfmtConfig::bookends`]).
pub const SEXPFMT_BOOKENDS_KEEP: c_int = -1;
/// Normalize all bookends to `( )`.
pub const SEXPFMT_BOOKENDS_PARENS: c_int = 0;
/// Normalize all bookends to `[ ]`.
pub const SEXPFMT_BOOKENDS_SQUARE: c_int = 1;
/// Normalize all bookends to `{ }`.
pub const SEXPFMT_BOOKENDS_CURLY: c_int = 2;

/// C mirror of [`Config`]; `sexpfmt_config` in `include/sexpfmt.h`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SexpfmtConfig {
	/// Number of spaces per indentation level.
	pub indent_width: usize,
	/// Target maximum line width.
	pub margin_width: usize,
	/// One of the `SEXPFMT_BOOKENDS_*` constants.
	pub bookends: c_int,
	/// Preserve `;` line comments instead of discarding them.
	pub preserve_comments: bool,
	/// In multi-line lists, keep a `:label` atom on the same line as the
	/// element that follows it.
	pub pair_labels: bool,
}

impl SexpfmtConfig {
	fn to_config(self) -> Option<Config> {
		let bookends = match self.bookends {
			SEXPFMT_BOOKENDS_KEEP => None,
			SEXPFMT_BOOKENDS_PARENS => Some(SExpBookendStyle::Parentheses),
			SEXPFMT_BOOKENDS_SQUARE => Some(SExpBookendStyle::SquareBrackets),
			SEXPFMT_BOOKENDS_CURLY => Some(SExpBookendStyle::CurlyBraces),
			_ => return None,
		};
		let mut config = Config::default();
		config.parser.preserve_comments = self.preserve_comments;
		config.printer.indent_width = self.indent_width;
		config.printer.margin_width = self.margin_width;
		config.printer.bookends = bookends;
		config.printer.pair_labels = self.pair_labels;
		Some(config)
	}
}

/// The default configuration: the same defaults as the CLI.
#[unsafe(no_mangle)]
pub extern "C" fn sexpfmt_config_default() -> SexpfmtConfig {
	SexpfmtConfig {
		indent_width: 2,
		margin_width: 80,
		bookends: SEXPFMT_BOOKENDS_KEEP,
		preserve_comments: false,
		pair_labels: false,
	}
}

/// Format `input_len` bytes of S-expression source from `input`.
///
/// On success, returns [`SEXPFMT_OK`] and stores a NUL-terminated, `malloc`ed
/// UTF-8 buffer in `*out`. The formatted text can contain interior NUL bytes
/// (a string literal may contain a literal NUL), so its exact byte length is
/// also stored in `*out_len` when `out_len` is non-null.
///
/// On failure, returns a nonzero error code, `*out` is null, and when `error`
/// is non-null, `*error` holds a NUL-terminated, `malloc`ed message.
///
/// Both `*out` and `*error` must be released with [`sexpfmt_str_free`].
/// `config` may be null, in which case [`sexpfmt_config_default`] applies.
///
/// # Safety
///
/// - `input` must point to `input_len` readable bytes (it may be null only
///   when `input_len` is 0; it need not be NUL-terminated).
/// - `config`, when non-null, must point to a valid [`SexpfmtConfig`].
/// - `out` must be a valid, non-null pointer to a `char *`.
/// - `out_len` and `error`, when non-null, must be valid to write through.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sexpfmt_format(
	input: *const c_char,
	input_len: usize,
	config: *const SexpfmtConfig,
	out: *mut *mut c_char,
	out_len: *mut usize,
	error: *mut *mut c_char,
) -> c_int {
	unsafe {
		if !out.is_null() {
			*out = ptr::null_mut();
		}
		if !out_len.is_null() {
			*out_len = 0;
		}
		if !error.is_null() {
			*error = ptr::null_mut();
		}
		if out.is_null() || (input.is_null() && input_len != 0) {
			set_error(error, "invalid argument: null `out` or `input`");
			return SEXPFMT_ERR_INVALID_ARGUMENT;
		}
		let config = if config.is_null() {
			Config::default()
		} else {
			match (*config).to_config() {
				Some(config) => config,
				None => {
					set_error(error, "invalid argument: unknown `bookends` value");
					return SEXPFMT_ERR_INVALID_ARGUMENT;
				}
			}
		};
		let input: &[u8] = if input_len == 0 {
			&[]
		} else {
			std::slice::from_raw_parts(input as *const u8, input_len)
		};
		let mut buf = Vec::new();
		match crate::format(input, &mut buf, &config) {
			Ok(()) => {
				let formatted = malloc_bytes(&buf);
				if formatted.is_null() {
					set_error(error, "out of memory");
					return SEXPFMT_ERR_NOMEM;
				}
				*out = formatted;
				if !out_len.is_null() {
					*out_len = buf.len();
				}
				SEXPFMT_OK
			}
			Err(e) => {
				set_error(error, &e.to_string());
				SEXPFMT_ERR_FORMAT
			}
		}
	}
}

/// Release a string returned by this API (in `*out` or `*error`).
/// Passing null is a no-op.
#[unsafe(no_mangle)]
pub extern "C" fn sexpfmt_str_free(s: *mut c_char) {
	if !s.is_null() {
		unsafe { libc::free(s as *mut libc::c_void) }
	}
}

/// Copy `bytes` into a fresh `malloc`ed buffer with a NUL terminator.
/// Returns null if allocation fails.
unsafe fn malloc_bytes(bytes: &[u8]) -> *mut c_char {
	unsafe {
		let ptr = libc::malloc(bytes.len() + 1) as *mut u8;
		if ptr.is_null() {
			return ptr::null_mut();
		}
		ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
		*ptr.add(bytes.len()) = 0;
		ptr as *mut c_char
	}
}

/// Store `message` in `*error` (when `error` is non-null) as a `malloc`ed,
/// NUL-terminated string. Interior NUL bytes — possible when an error message
/// quotes a NUL from the input — are replaced so that the message reads
/// correctly as a C string.
unsafe fn set_error(error: *mut *mut c_char, message: &str) {
	if error.is_null() {
		return;
	}
	let sanitized = message.replace('\0', "\u{FFFD}");
	unsafe {
		*error = malloc_bytes(sanitized.as_bytes());
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::ffi::CStr;

	/// Run `sexpfmt_format` and return `(code, out, out_len, error)`, freeing
	/// the C allocations.
	fn run(
		input: &[u8],
		config: Option<SexpfmtConfig>,
	) -> (c_int, Option<Vec<u8>>, usize, Option<String>) {
		let config_ptr = config
			.as_ref()
			.map_or(ptr::null(), |config| config as *const SexpfmtConfig);
		let mut out: *mut c_char = ptr::null_mut();
		let mut out_len: usize = 0;
		let mut error: *mut c_char = ptr::null_mut();
		let code = unsafe {
			sexpfmt_format(
				input.as_ptr() as *const c_char,
				input.len(),
				config_ptr,
				&mut out,
				&mut out_len,
				&mut error,
			)
		};
		let out_bytes = if out.is_null() {
			None
		} else {
			let bytes = unsafe { std::slice::from_raw_parts(out as *const u8, out_len) }.to_vec();
			sexpfmt_str_free(out);
			Some(bytes)
		};
		let error_text = if error.is_null() {
			None
		} else {
			let text = unsafe { CStr::from_ptr(error) }
				.to_str()
				.unwrap()
				.to_string();
			sexpfmt_str_free(error);
			Some(text)
		};
		(code, out_bytes, out_len, error_text)
	}

	#[test]
	fn test_format_with_default_config() {
		let (code, out, out_len, error) = run(b"(a  b)", None);
		assert_eq!(code, SEXPFMT_OK);
		assert_eq!(out.as_deref(), Some(&b"(a b)\n"[..]));
		assert_eq!(out_len, 6);
		assert_eq!(error, None);
	}

	#[test]
	fn test_format_with_custom_config() {
		let mut config = sexpfmt_config_default();
		config.bookends = SEXPFMT_BOOKENDS_PARENS;
		config.preserve_comments = true;
		let (code, out, _, _) = run(b"[x ; note\n y]", Some(config));
		assert_eq!(code, SEXPFMT_OK);
		assert_eq!(out.as_deref(), Some(&b"(x\n  ; note\n  y)\n"[..]));
	}

	#[test]
	fn test_parse_error_is_reported() {
		let (code, out, _, error) = run(b"(a", None);
		assert_eq!(code, SEXPFMT_ERR_FORMAT);
		assert_eq!(out, None);
		assert!(error.unwrap().contains("Unexpected EOF"));
	}

	#[test]
	fn test_invalid_bookends_value_is_rejected() {
		let mut config = sexpfmt_config_default();
		config.bookends = 42;
		let (code, out, _, error) = run(b"(a)", Some(config));
		assert_eq!(code, SEXPFMT_ERR_INVALID_ARGUMENT);
		assert_eq!(out, None);
		assert!(error.unwrap().contains("bookends"));
	}

	#[test]
	fn test_out_len_covers_interior_nul() {
		// A literal NUL inside a string literal is preserved in the output, so
		// `out_len` (not `strlen`) is the true output length.
		let (code, out, out_len, _) = run(b"\"a\0b\"", None);
		assert_eq!(code, SEXPFMT_OK);
		assert_eq!(out.as_deref(), Some(&b"\"a\0b\"\n"[..]));
		assert_eq!(out_len, 6);
	}

	#[test]
	fn test_null_out_is_invalid() {
		let code = unsafe {
			sexpfmt_format(
				b"(a)".as_ptr() as *const c_char,
				3,
				ptr::null(),
				ptr::null_mut(),
				ptr::null_mut(),
				ptr::null_mut(),
			)
		};
		assert_eq!(code, SEXPFMT_ERR_INVALID_ARGUMENT);
	}
}
