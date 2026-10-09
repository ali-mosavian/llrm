//! What an inline copy of a routine computes, and how the runtime reads a
//! string, read from `runtime.toml`'s `semantics` and `layout` tables.

use std::sync::LazyLock;

use llrm_hir::meaning::{Arithmetic, Descriptor, Expr, Form, Meaning, Parameter, Returns};
use llrm_hir::model::StackCheck;

static TABLE: LazyLock<toml::Table> = LazyLock::new(|| super::TABLE.parse().expect("runtime.toml parses"));

/// How `family`'s runtime keeps its strings.
pub fn form(family: &str) -> Option<Form> {
    match TABLE.get("layout")?.get(family)?.get("form")?.as_str()? {
        "near" => Some(Form::Near),
        "far" => Some(Form::Far),
        other => panic!("layout.{family}.form is {other}"),
    }
}

/// How `family`'s runtime reads a string descriptor; none where its strings are not near, and its
/// routines keep their calls.
pub fn descriptor(family: &str) -> Option<Descriptor> {
    if form(family)? != Form::Near {
        return None;
    }
    let row = TABLE.get("layout")?.get(family)?;
    let at =
        |key: &str| row.get(key).and_then(toml::Value::as_integer).unwrap_or_else(|| panic!("layout.{family}.{key}"));
    Some(Descriptor { length: at("length"), data: at("data"), size: at("size") })
}

/// What `family`'s runtime says of its stack, where it checks one.
pub fn stack(family: &str) -> Option<StackCheck> {
    let row = TABLE.get("stack")?.get(family)?;
    Some(StackCheck::from_toml(row).unwrap_or_else(|why| panic!("stack.{family}: {why}")))
}

/// Each routine the table describes.
pub fn routines() -> Vec<Meaning> {
    let Some(rows) = TABLE.get("semantics").and_then(toml::Value::as_table) else { return Vec::new() };
    rows.iter().map(|(name, row)| meaning(name, row).unwrap_or_else(|why| panic!("semantics.{name}: {why}"))).collect()
}

fn meaning(
    name: &str,
    row: &toml::Value,
) -> Result<Meaning, String> {
    let text = |value: Option<&toml::Value>, what: &str| {
        value.and_then(toml::Value::as_str).map(str::to_owned).ok_or_else(|| format!("{what} is not a string"))
    };
    let parameters = row
        .get("parameters")
        .and_then(toml::Value::as_array)
        .ok_or("parameters is not a list")?
        .iter()
        .map(|one| match one.as_str() {
            Some("string") => Ok(Parameter::String),
            Some("int") => Ok(Parameter::Int),
            other => Err(format!("a parameter of kind {other:?}")),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let result = match (row.get("result"), row.get("view")) {
        (Some(value), None) => Returns::Value(expression(&text(Some(value), "result")?)?),
        (None, Some(view)) => Returns::View {
            length: expression(&text(view.get("length"), "view.length")?)?,
            data: expression(&text(view.get("data"), "view.data")?)?,
        },
        _ => return Err("states one of result and view".to_owned()),
    };
    let releases = row
        .get("releases")
        .and_then(toml::Value::as_array)
        .map_or(Vec::new(), |list| list.iter().filter_map(|one| one.as_integer()).map(|one| one as usize).collect());
    let check = match row.get("check") {
        Some(check) => Some((
            expression(&text(check.get("when"), "check.when")?)?,
            check.get("raise").and_then(toml::Value::as_integer).ok_or("check.raise is not an integer")?,
        )),
        None => None,
    };
    Ok(Meaning { routine: name.to_owned(), parameters, result, releases, check })
}

/// `text` as an expression: `or` over `==` and `<` over `+` and `-` over
/// calls of `len`, `data`, `byte`, `slot`, `min` and `max`, parameters `$n` and
/// integers.
pub fn expression(text: &str) -> Result<Expr, String> {
    let tokens = tokens(text)?;
    let mut at = 0;
    let expr = parse_or(&tokens, &mut at)?;
    if at != tokens.len() {
        return Err(format!("{:?} follows the expression {text:?}", tokens[at]));
    }
    Ok(expr)
}

fn tokens(text: &str) -> Result<Vec<String>, String> {
    let (mut out, chars): (Vec<String>, Vec<char>) = (Vec::new(), text.chars().collect());
    let mut at = 0;
    // The run of characters from `from` that `keep` holds of.
    let run = |from: usize, keep: fn(char) -> bool| from + chars[from..].iter().take_while(|&&one| keep(one)).count();
    while at < chars.len() {
        let c = chars[at];
        let stop = match c {
            ' ' => {
                at += 1;
                continue;
            }
            '$' => run(at + 1, |one| one.is_ascii_digit()),
            '0'..='9' => run(at, |one| one.is_ascii_digit()),
            'a'..='z' => run(at, |one| one.is_ascii_lowercase()),
            '=' if chars.get(at + 1) == Some(&'=') => at + 2,
            '+' | '-' | '<' | '(' | ')' | ',' => at + 1,
            _ => return Err(format!("{c:?} in {text:?}")),
        };
        out.push(chars[at..stop].iter().collect());
        at = stop;
    }
    Ok(out)
}

fn parse_or(
    tokens: &[String],
    at: &mut usize,
) -> Result<Expr, String> {
    let mut left = parse_compare(tokens, at)?;
    while tokens.get(*at).is_some_and(|one| one == "or") {
        *at += 1;
        left = Expr::Binary(Arithmetic::Or, Box::new(left), Box::new(parse_compare(tokens, at)?));
    }
    Ok(left)
}

fn parse_compare(
    tokens: &[String],
    at: &mut usize,
) -> Result<Expr, String> {
    let left = parse_sum(tokens, at)?;
    let op = match tokens.get(*at).map(String::as_str) {
        Some("==") => Arithmetic::Eq,
        Some("<") => Arithmetic::Lt,
        _ => return Ok(left),
    };
    *at += 1;
    Ok(Expr::Binary(op, Box::new(left), Box::new(parse_sum(tokens, at)?)))
}

fn parse_sum(
    tokens: &[String],
    at: &mut usize,
) -> Result<Expr, String> {
    let mut left = parse_atom(tokens, at)?;
    while let Some(op) = tokens.get(*at).and_then(|one| match one.as_str() {
        "+" => Some(Arithmetic::Add),
        "-" => Some(Arithmetic::Sub),
        _ => None,
    }) {
        *at += 1;
        left = Expr::Binary(op, Box::new(left), Box::new(parse_atom(tokens, at)?));
    }
    Ok(left)
}

fn parse_atom(
    tokens: &[String],
    at: &mut usize,
) -> Result<Expr, String> {
    let token = tokens.get(*at).ok_or("the expression ends early")?.clone();
    *at += 1;
    if token == "(" {
        let inside = parse_or(tokens, at)?;
        expect(tokens, at, ")")?;
        return Ok(inside);
    }
    if let Some(number) = token.strip_prefix('$') {
        return number.parse().map(Expr::Param).map_err(|_| format!("{token:?} names no parameter"));
    }
    if let Ok(value) = token.parse() {
        return Ok(Expr::Int(value));
    }
    expect(tokens, at, "(")?;
    let mut arguments = vec![parse_or(tokens, at)?];
    while tokens.get(*at).is_some_and(|one| one == ",") {
        *at += 1;
        arguments.push(parse_or(tokens, at)?);
    }
    expect(tokens, at, ")")?;
    let parameter = |expr: &Expr| match expr {
        Expr::Param(n) => Ok(*n),
        other => Err(format!("{token} reads a parameter, not {other:?}")),
    };
    match (token.as_str(), arguments.as_slice()) {
        ("len", [one]) => Ok(Expr::Length(parameter(one)?)),
        ("data", [one]) => Ok(Expr::Data(parameter(one)?)),
        ("byte", [one]) => Ok(Expr::Byte(Box::new(one.clone()))),
        ("slot", [one]) => Ok(Expr::Slot(Box::new(one.clone()))),
        ("min", [a, b]) => Ok(Expr::Binary(Arithmetic::Min, Box::new(a.clone()), Box::new(b.clone()))),
        ("max", [a, b]) => Ok(Expr::Binary(Arithmetic::Max, Box::new(a.clone()), Box::new(b.clone()))),
        _ => Err(format!("{token} of {} arguments", arguments.len())),
    }
}

fn expect(
    tokens: &[String],
    at: &mut usize,
    want: &str,
) -> Result<(), String> {
    if tokens.get(*at).is_some_and(|one| one == want) {
        *at += 1;
        Ok(())
    } else {
        Err(format!("{want:?} expected, found {:?}", tokens.get(*at)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A routine whose row does not parse would silently keep its call; every row loads.
    #[test]
    fn every_described_routine_loads() {
        let names: Vec<_> = routines().into_iter().map(|one| one.routine).collect();
        assert_eq!(names, ["B$FLEN", "B$FASC", "B$FMID", "B$FCHR"]);
    }

    /// QB 4.5 and PDS 7.1 state their descriptor; VBDOS's far strings are not at an offset, so it
    /// states none and keeps every call.
    #[test]
    fn the_far_runtime_states_no_descriptor() {
        assert_eq!(descriptor("qb45"), Some(Descriptor { length: 0, data: 2, size: 4 }));
        assert_eq!(descriptor("pds71"), descriptor("qb45"));
        assert_eq!(descriptor("vbdos"), None);
        assert_eq!((form("qb45"), form("pds71"), form("vbdos")), (Some(Form::Near), Some(Form::Near), Some(Form::Far)));
    }

    /// Every BASIC runtime states its limit word and handler; a pass naming `b$pendchk` itself
    /// would not notice a runtime that keeps it elsewhere.
    #[test]
    fn each_basic_runtime_states_its_stack_limit() {
        for family in ["qb45", "pds71", "vbdos"] {
            let check = stack(family).unwrap_or_else(|| panic!("{family} states no stack"));
            assert_eq!(
                (check.limit.as_str(), check.handler.as_str(), check.entry.as_deref()),
                ("b$pendchk", "B$ERR_OSS", Some("B$ENRD")),
                "{family}"
            );
        }
        assert_eq!(stack("freestanding"), None);
    }

    #[test]
    fn an_expression_reads_as_written() {
        let one = |n| Box::new(Expr::Int(n));
        assert_eq!(
            expression("$1 - 1 + 2").unwrap(),
            Expr::Binary(
                Arithmetic::Add,
                Box::new(Expr::Binary(Arithmetic::Sub, Box::new(Expr::Param(1)), one(1))),
                one(2)
            )
        );
        assert!(expression("len($0) == 0").is_ok() && expression("byte(data($0) + ($1 - 1))").is_ok());
        assert!(expression("len(1)").is_err() && expression("$0 $1").is_err());
    }
}
