//! Open Watcom capture-stream frontend.

pub mod capture;
pub mod raise;

pub use raise::{RaiseError, RaiseErrorKind, raise_module};

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Record {
    pub line: usize,
    pub result: Option<String>,
    pub call: String,
    pub args: Vec<String>,
    pub fields: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseError {
    pub line: usize,
    pub column: usize,
    pub kind: ParseErrorKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParseErrorKind {
    MissingCall,
    UnterminatedQuotedValue,
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "capture stream line {}, column {}: ",
            self.line, self.column
        )?;
        match self.kind {
            ParseErrorKind::MissingCall => {
                formatter.write_str("expected a call after the result marker")
            }
            ParseErrorKind::UnterminatedQuotedValue => {
                formatter.write_str("unterminated quoted value")
            }
        }
    }
}

impl Error for ParseError {}

/// Parses the line-oriented stream written by owshim's `cgshim.c`.
///
/// A leading lower-case letter followed by digits is a result handle; `-`
/// explicitly marks a call that returned no handle.  Values are separated by
/// literal spaces, except that spaces inside double quotes stay in one value.
pub fn parse(text: &str) -> Result<Vec<Record>, ParseError> {
    let mut records = Vec::new();

    for (line_index, line) in text.lines().enumerate() {
        let line_number = line_index + 1;
        let tokens = tokens(line, line_number)?;
        if tokens.is_empty() {
            continue;
        }

        let (result, call_index) = match tokens[0].text.as_str() {
            "-" => (None, 1),
            token if is_handle(token) => (Some(token.to_owned()), 1),
            _ => (None, 0),
        };
        let Some(call_token) = tokens.get(call_index) else {
            return Err(ParseError {
                line: line_number,
                column: tokens[0].column,
                kind: ParseErrorKind::MissingCall,
            });
        };

        let mut args = Vec::new();
        let mut fields = BTreeMap::new();
        for token in &tokens[call_index + 1..] {
            if let Some((key, value)) = token.text.split_once('=') {
                if is_identifier(key) {
                    fields.insert(key.to_owned(), decode_value(value));
                    continue;
                }
            }
            args.push(decode_value(&token.text));
        }

        records.push(Record {
            line: line_number,
            result,
            call: call_token.text.clone(),
            args,
            fields,
        });
    }

    Ok(records)
}

#[derive(Clone, Debug)]
struct Token {
    text: String,
    column: usize,
}

fn tokens(line: &str, line_number: usize) -> Result<Vec<Token>, ParseError> {
    let characters: Vec<char> = line.chars().collect();
    let mut tokens = Vec::new();
    let mut index = 0;

    while index < characters.len() {
        if characters[index] == ' ' {
            index += 1;
            continue;
        }

        let start = index;
        while index < characters.len() && characters[index] != ' ' {
            if characters[index] == '"' {
                let quote_column = index + 1;
                index += 1;
                while index < characters.len() && characters[index] != '"' {
                    index += 1;
                }
                if index == characters.len() {
                    return Err(ParseError {
                        line: line_number,
                        column: quote_column,
                        kind: ParseErrorKind::UnterminatedQuotedValue,
                    });
                }
            }
            index += 1;
        }
        tokens.push(Token {
            text: characters[start..index].iter().collect(),
            column: start + 1,
        });
    }

    Ok(tokens)
}

fn is_handle(token: &str) -> bool {
    let mut characters = token.chars();
    matches!(characters.next(), Some(letter) if letter.is_ascii_lowercase())
        && characters
            .next()
            .is_some_and(|digit| digit.is_ascii_digit())
        && characters.all(|digit| digit.is_ascii_digit())
}

fn is_identifier(text: &str) -> bool {
    let mut characters = text.chars();
    matches!(characters.next(), Some(character) if character == '_' || character.is_alphabetic())
        && characters.all(|character| character == '_' || character.is_alphanumeric())
}

fn decode_value(token: &str) -> String {
    if !token.starts_with('"') || !token.ends_with('"') || token.len() < 2 {
        return token.to_owned();
    }

    let content = &token[1..token.len() - 1];
    let characters: Vec<char> = content.chars().collect();
    let mut value = String::new();
    let mut index = 0;
    while index < characters.len() {
        if characters[index] == '\\' && characters.get(index + 1) == Some(&'x') {
            if let (Some(&high), Some(&low)) =
                (characters.get(index + 2), characters.get(index + 3))
            {
                if high.is_ascii_hexdigit()
                    && !high.is_ascii_uppercase()
                    && low.is_ascii_hexdigit()
                    && !low.is_ascii_uppercase()
                {
                    let byte = high.to_digit(16).unwrap() * 16 + low.to_digit(16).unwrap();
                    value.push(char::from(byte as u8));
                    index += 4;
                    continue;
                }
            }
        }
        value.push(characters[index]);
        index += 1;
    }
    value
}

#[cfg(test)]
mod tests {
    use super::{ParseError, ParseErrorKind, Record, parse};
    use std::collections::BTreeMap;

    #[test]
    fn records_a_result_handle_and_call() {
        assert_eq!(
            parse("n7 CGBinary O_PLUS n5 n6 TY_INTEGER\n").unwrap(),
            vec![Record {
                line: 1,
                result: Some("n7".to_owned()),
                call: "CGBinary".to_owned(),
                args: vec![
                    "O_PLUS".to_owned(),
                    "n5".to_owned(),
                    "n6".to_owned(),
                    "TY_INTEGER".to_owned()
                ],
                fields: BTreeMap::new(),
            }],
        );
    }

    #[test]
    fn keeps_no_result_calls_and_directive_calls_distinct_from_handles() {
        let records = parse("- CGDone n7\nSTART\n").unwrap();

        assert_eq!(records[0].line, 1);
        assert_eq!(records[0].result, None);
        assert_eq!(records[0].call, "CGDone");
        assert_eq!(records[0].args, ["n7"]);
        assert_eq!(records[1].line, 2);
        assert_eq!(records[1].result, None);
        assert_eq!(records[1].call, "START");
    }

    #[test]
    fn separates_fields_from_positional_values_and_unescapes_quoted_values() {
        let records = parse(
            "SYM y1 name=\"pal now\\x21\" attr=0x42 1bad=value\n\
             f1 DBSrcFile \"fixtures/c\\x2fpal now.c\"\n",
        )
        .unwrap();

        assert_eq!(records[0].args, ["y1", "1bad=value"]);
        assert_eq!(records[0].fields["name"], "pal now!");
        assert_eq!(records[0].fields["attr"], "0x42");
        assert_eq!(records[1].result.as_deref(), Some("f1"));
        assert_eq!(records[1].args, ["fixtures/c/pal now.c"]);
    }

    #[test]
    fn skips_blank_lines_without_renumbering_records() {
        let records = parse("\n   \n- CGDone n7\n").unwrap();

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].line, 3);
    }

    #[test]
    fn preserves_unrecognized_escapes_like_the_capture_parser() {
        let records = parse("- DGBytes \"bad\\x4 and \\xAF\"\n").unwrap();
        assert_eq!(records[0].args, ["bad\\x4 and \\xAF"]);
    }

    #[test]
    fn reports_malformed_structure_with_locations() {
        for (text, line, column, kind) in [
            (
                "f1 DBSrcFile \"missing\n",
                1,
                14,
                ParseErrorKind::UnterminatedQuotedValue,
            ),
            ("-\n", 1, 1, ParseErrorKind::MissingCall),
        ] {
            assert_eq!(parse(text), Err(ParseError { line, column, kind }),);
        }
    }

    #[test]
    fn parses_a_real_wcc_capture_without_source_specific_cases() {
        let records = parse(include_str!("../../../fixtures/c/choose.cgs")).unwrap();

        assert_eq!(records.len(), 127);
        assert_eq!(records[0].call, "INIT");
        assert_eq!(records[0].fields["target"], "0xec");
        assert_eq!(records[1].args, ["1"]);
        assert_eq!(records[1].fields["name"], "_TEXT");
        assert_eq!(records.last().unwrap().call, "FINI");
    }
}
