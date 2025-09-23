mod error;

mod parser;
mod printer;
mod reader;
mod sexp;

pub use error::*;
pub use parser::*;
pub use printer::*;
pub use reader::*;
pub use sexp::*;

#[cfg(test)]
mod error_tests {
	use super::*;

	#[test]
	fn test_io_error_conversion() {
		let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
		let sexp_err: SexpfmtError = io_err.into();

		match sexp_err {
			SexpfmtError::Io { source } => {
				assert_eq!(source.kind(), std::io::ErrorKind::NotFound);
			}
			_ => panic!("Expected Io error"),
		}
	}

	#[test]
	fn test_utf8_error_conversion() {
		let invalid_utf8 = vec![0, 159, 146, 150];
		let utf8_err = String::from_utf8(invalid_utf8).unwrap_err();
		let sexp_err: SexpfmtError = utf8_err.into();

		match sexp_err {
			SexpfmtError::Utf8 { source: _ } => {
				// Success
			}
			_ => panic!("Expected Utf8 error"),
		}
	}

	#[test]
	fn test_form_reader_error_with_position() {
		let position = Loc::new(42, 3, 15);

		let err = SexpfmtError::form_reader_error("Test error message", Some(position.clone()), None);

		let display_str = format!("{}", err);
		assert!(display_str.contains("Form reader error at line 3, column 15 (offset 42)"));
		assert!(display_str.contains("Test error message"));
	}

	#[test]
	fn test_form_reader_error_without_position() {
		let err = SexpfmtError::form_reader_error("Test error message", None, None);

		let display_str = format!("{}", err);
		assert!(display_str.contains("Form reader error: Test error message"));
		assert!(!display_str.contains("line"));
		assert!(!display_str.contains("column"));
	}

	#[test]
	fn test_parse_error_with_position() {
		let position = Loc::new(10, 2, 5);

		let err = SexpfmtError::parse_error("Unexpected token", position.clone(), None);

		let display_str = format!("{}", err);
		assert!(display_str.contains("Parse error at line 2, column 5 (offset 10)"));
		assert!(display_str.contains("Unexpected token"));
	}

	#[test]
	fn test_mismatched_bookends_error() {
		let position = Loc::new(5, 1, 6);

		let err = SexpfmtError::mismatched_bookends(
			position.clone(),
			SExpBookendStyle::Parentheses,
			SExpBookendStyle::SquareBrackets,
		);

		let display_str = format!("{}", err);
		assert!(display_str.contains("Mismatched bookends at line 1, column 6 (offset 5)"));
		assert!(display_str.contains("got Parentheses"));
		assert!(display_str.contains("expected SquareBrackets"));
	}

	#[test]
	fn test_unexpected_eof_error() {
		let position = Loc::new(100, 5, 1);

		let err = SexpfmtError::unexpected_eof(position.clone(), 3);

		let display_str = format!("{}", err);
		assert!(display_str.contains("Unexpected EOF at line 5, column 1 (offset 100)"));
		assert!(display_str.contains("3 unclosed bookends"));
	}

	#[test]
	fn test_invalid_input_error() {
		let position = Loc::new(25, 3, 10);

		let err = SexpfmtError::invalid_input("Invalid character '@'", position.clone());

		let display_str = format!("{}", err);
		assert!(display_str.contains("Invalid input at line 3, column 10 (offset 25)"));
		assert!(display_str.contains("Invalid character '@'"));
	}

	#[test]
	fn test_line_col_display() {
		let position = Loc::new(123, 10, 5);

		let display_str = format!("{}", position);
		assert_eq!(display_str, "line 10, column 5 (offset 123)");
	}

	#[test]
	fn test_error_with_source_chain() {
		let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "Access denied");
		let position = Loc::new(0, 1, 1);

		let err = SexpfmtError::form_reader_error(
			"Failed to read input",
			Some(position),
			Some(Box::new(io_err)),
		);

		// Test that we can access the source chain
		use std::error::Error;
		assert!(err.source().is_some());

		let display_str = format!("{}", err);
		assert!(display_str.contains("Form reader error at line 1, column 1 (offset 0)"));
		assert!(display_str.contains("Failed to read input"));
	}

	#[test]
	fn test_result_type_alias() {
		// Test that our Result type alias works correctly
		fn test_function() -> Result<String> {
			Ok("success".to_string())
		}

		let result = test_function();
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), "success");
	}

	#[test]
	fn test_error_conversion_from_std_errors() {
		// Test automatic conversion from std::io::Error
		fn io_operation() -> Result<()> {
			std::fs::read_to_string("nonexistent_file.txt")?;
			Ok(())
		}

		let result = io_operation();
		assert!(result.is_err());

		if let Err(SexpfmtError::Io { source: _ }) = result {
			// Success - the conversion worked
		} else {
			panic!("Expected automatic conversion to SexpfmtError::Io");
		}
	}
}
