//! The core error reporting structs and utility functions for
//! reporting different kinds of errors.
use std::io;

use crate::result::Result;
use std::fmt::Display;
use std::ops::Range;

const UNEXPECTED_TOKEN_ERROR_TITLE: &str = "Unexpected Token";
const ILLEGAL_FUNCTION_CALLEE_TITLE: &str = "Illegal Function Call";
const UNEXPECTED_CHARACTER_ERROR_TITLE: &str = "Unexpected Character";
const EMPTY_TYPE_PARAMETERS: &str = "Type parameters cannot be empty";
const UNKNOWN_REFERENCE: &str = "Unknown Reference";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    message: String,
    range: Range<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    message: String,
    labels: Vec<Label>,
    notes: Option<Vec<String>>,
}

impl Diagnostic {
    pub fn error(message: String, labels: Vec<Label>) -> Diagnostic {
        Self {
            message,
            labels,
            notes: None,
        }
    }

    fn with_note(self, note: impl Into<String>) -> Self {
        let mut notes = self.notes.unwrap_or_default();
        notes.push(note.into());
        Self {
            message: self.message,
            labels: self.labels,
            notes: Some(notes),
        }
    }
}

/// Takes an instance of our own `Diagnostic` and converts it to the `codespan_reporting` variant
/// so we can report the error in the terminal.
pub fn report_diagnostic_to_term(diagnostic: Diagnostic, file_name: &str, file_source: &str) {
    use codespan_reporting::diagnostic::{
        Diagnostic as CodespanDiagnostic, Label as CodespanLabel,
    };
    use codespan_reporting::files::SimpleFiles;
    use codespan_reporting::term;
    use codespan_reporting::term::termcolor::{ColorChoice, StandardStream};
    let mut files = SimpleFiles::new();
    // Map our diagnostic to the codespan structures
    let diagnostic = {
        let id = files.add(file_name, file_source);
        let labels = diagnostic
            .labels
            .iter()
            .map(|label| {
                // We track ranges as fully inclusive as that is easier for lexing,
                // but technically `std::ops::Range` in Rust is only inclusive for
                // the start of the range. We shift the end of the range out by one
                // to account for this.
                let range = label.range.start..label.range.end + 1;
                CodespanLabel::primary(id, range).with_message(label.message.clone())
            })
            .collect();
        let mut csp_diagnostic = CodespanDiagnostic::error()
            .with_message(diagnostic.message)
            .with_labels(labels);
        if let Some(notes) = diagnostic.notes {
            csp_diagnostic = csp_diagnostic.with_notes(notes)
        }
        csp_diagnostic
    };
    let writer = StandardStream::stderr(ColorChoice::Always);
    let mut writer = writer.lock();
    let config = codespan_reporting::term::Config::default();
    term::emit(&mut writer, &config, &files, &diagnostic).unwrap()
}

/// Report an unexpected token error for the parser
pub fn unexpected_token_error<T>(
    span: impl Into<Range<usize>>,
    expected: impl Display,
    found: impl Display,
) -> Result<T> {
    let label = Label {
        message: format!("Expected '{}' but found '{}'", expected, found),
        range: span.into(),
    };

    let diagnostic = Diagnostic::error(UNEXPECTED_TOKEN_ERROR_TITLE.into(), vec![label]);
    Err(crate::error::Error::Diagnostic(diagnostic))
}

/// Report an unexpected token error for the parser
pub fn illegal_function_callee<T>(
    span: impl Into<Range<usize>>,
) -> Result<T> {
    let label = Label {
        message: format!("You can't call this as a function, dumb bitch"),
        range: span.into(),
    };
    let diagnostic = Diagnostic::error(ILLEGAL_FUNCTION_CALLEE_TITLE.into(), vec![label]);
    Err(crate::error::Error::Diagnostic(diagnostic))
}

/// Report an unknown reference error for the parser
pub fn unknown_reference_error<T>(
    span: impl Into<Range<usize>>,
    name: impl Display,
) -> Result<T> {
    let label = Label {
        message: format!("Cannot resolve '{}'", name),
        range: span.into(),
    };

    let diagnostic = Diagnostic::error(UNKNOWN_REFERENCE.into(), vec![label]);
    Err(crate::error::Error::Diagnostic(diagnostic))
}

/// Report an unexpected token error where multiple expected tokens are possible
pub fn unexpected_token_error_with_multiple_options<T>(
    span: impl Into<Range<usize>>,
    expected: Vec<impl Display>,
    found: impl Display,
) -> Result<T> {
    let message = match expected.split_last() {
        Some((last, rest)) => {
            let rest = rest
                .iter()
                .map(|token| format!("'{}'", token))
                .collect::<Vec<String>>()
                .join(", ");
            format!("Expected {} or '{}' but found '{}'", rest, last, found)
        }
        None => "".into(),
    };
    let label = Label {
        message,
        range: span.into(),
    };
    Err(crate::error::Error::Diagnostic(
        Diagnostic::error(UNEXPECTED_TOKEN_ERROR_TITLE.into(), vec![label])
            .with_note("We were attempting to parse a top-level item"),
    ))
}

/// Report an unexpected token error for the parser
pub fn expected_identifier<T>(span: impl Into<Range<usize>>, found: impl Display) -> Result<T> {
    let label = Label {
        message: format!("Expected an identifier but found '{}'", found),
        range: span.into(),
    };
    let diagnostic = Diagnostic::error(UNEXPECTED_TOKEN_ERROR_TITLE.into(), vec![label]);
    Err(crate::error::Error::Diagnostic(diagnostic))
}

/// Report an invalid character
pub fn invalid_character<T>(span: impl Into<Range<usize>>) -> Result<T> {
    let label = Label {
        message: "This character isn't recognized".into(),
        range: span.into(),
    };
    let diagnostic = Diagnostic::error(UNEXPECTED_CHARACTER_ERROR_TITLE.into(), vec![label]);
    Err(crate::error::Error::Diagnostic(diagnostic))
}

pub fn multiple_decimal_in_number<T>(span: impl Into<Range<usize>>) -> Result<T> {
    let label = Label {
        message: "You can't have multiple decimal points in a number".into(),
        range: span.into(),
    };
    let diagnostic = Diagnostic::error(UNEXPECTED_TOKEN_ERROR_TITLE.into(), vec![label]);
    Err(crate::error::Error::Diagnostic(diagnostic))
}


/// Report an empty type parameter list
pub fn empty_type_parameters<T>(span: impl Into<Range<usize>>) -> Result<T> {
    let label = Label {
        message: "".into(),
        range: span.into(),
    };
    Err(Error::Diagnostic(Diagnostic::error(
        EMPTY_TYPE_PARAMETERS.into(),
        vec![label],
    )))
}

pub fn dot_after_import_list<T>(span: impl Into<Range<usize>>) -> Result<T> {
    let label = Label {
        message: "".into(),
        range: span.into(),
    };
    Err(Error::Diagnostic(Diagnostic::error(
        "Cannot use identifier imports after lists".into(),
        vec![label],
    )))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    IO,
    Lexing,
    Diagnostic(Diagnostic),
}

impl From<io::Error> for Error {
    fn from(_err: io::Error) -> Self {
        Error::IO
    }
}
