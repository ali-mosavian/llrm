//! Python's format-spec mini-language, parsed and checked when an f-string
//! is compiled. Errors carry Python's own messages.

/// `[[fill]align][sign][z][#][0][width][grouping][.precision][type]`.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Spec {
    pub fill: Option<char>,
    pub align: Option<char>,
    pub sign: Option<char>,
    pub zeroless: bool,
    pub alternate: bool,
    pub zero: bool,
    pub width: u16,
    pub grouping: Option<char>,
    pub precision: Option<u16>,
    pub kind: Option<char>,
}

/// What the value being formatted is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Class {
    Text,
    Integer,
    Float,
}

/// A spec resolved against its value: which prelude routine lays it out.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Layout {
    /// `QUICKR_TEXT$`: a string, cut to `precision` (-1 for none).
    Text {
        fill: char,
        align: char,
        width: u16,
        precision: i32,
    },
    /// `QUICKR_NUMERIC$`: a float's plain text without a type or precision.
    Plain {
        sign: char,
        fill: char,
        align: char,
        width: u16,
        separator: String,
    },
    /// `QUICKR_FORMAT$`: a number under a presentation type; `kind` "" is
    /// a float's bare precision.
    Format {
        kind: String,
        sign: char,
        alternate: bool,
        zeroless: bool,
        fill: char,
        align: char,
        width: u16,
        separator: String,
        precision: u16,
    },
}

fn number(text: &str, at: &mut usize) -> Option<u16> {
    let digits = text[*at..].bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    let value = text[*at..*at + digits].parse().ok();
    *at += digits;
    value
}

pub(super) fn parse(text: &str) -> Result<Spec, String> {
    let invalid = || format!("Invalid format specifier '{text}'");
    let characters: Vec<char> = text.chars().collect();
    let aligns = ['<', '>', '=', '^'];
    let mut spec = Spec::default();
    let mut index = 0;
    if characters.len() >= 2 && aligns.contains(&characters[1]) {
        spec.fill = Some(characters[0]);
        spec.align = Some(characters[1]);
        index = 2;
    } else if characters.first().is_some_and(|one| aligns.contains(one)) {
        spec.align = Some(characters[0]);
        index = 1;
    }
    // The rest is ASCII: work in bytes from here.
    let mut at: usize = characters[..index].iter().map(|one| one.len_utf8()).sum();
    let bytes = text.as_bytes();
    let next = |at: usize| bytes.get(at).copied();
    if let Some(sign @ (b'+' | b'-' | b' ')) = next(at) {
        spec.sign = Some(sign as char);
        at += 1;
    }
    if next(at) == Some(b'z') {
        spec.zeroless = true;
        at += 1;
    }
    if next(at) == Some(b'#') {
        spec.alternate = true;
        at += 1;
    }
    if next(at) == Some(b'0') {
        spec.zero = true;
        at += 1;
    }
    spec.width = number(text, &mut at).unwrap_or(0);
    if let Some(grouping @ (b',' | b'_')) = next(at) {
        spec.grouping = Some(grouping as char);
        at += 1;
        if matches!(next(at), Some(b',' | b'_')) {
            return Err("Cannot specify both ',' and '_'.".into());
        }
    }
    if next(at) == Some(b'.') {
        at += 1;
        spec.precision = Some(number(text, &mut at).ok_or("Format specifier missing precision")?);
    }
    if let Some(kind) = next(at) {
        if !b"bcdeEfFgGnosxX%".contains(&kind) {
            return Err(invalid());
        }
        spec.kind = Some(kind as char);
        at += 1;
    }
    if at != bytes.len() {
        return Err(invalid());
    }
    Ok(spec)
}

/// `spec` for a value of `class`, or Python's complaint about the pair.
pub(super) fn layout(spec: &Spec, class: Class) -> Result<Layout, String> {
    let type_name = match class {
        Class::Text => "str",
        Class::Integer => "int",
        Class::Float => "float",
    };
    let kind = spec.kind;
    let unknown = |kind: char| format!("Unknown format code '{kind}' for object of type '{type_name}'");
    let separator = spec.grouping.map(String::from).unwrap_or_default();
    // A bare 0 zero-fills after the sign, unless an alignment is given.
    let (default_fill, default_align) = match (spec.zero, class) {
        (true, Class::Text) => ('0', '<'),
        (true, _) if spec.align.is_none() => ('0', '='),
        (_, Class::Text) => (' ', '<'),
        _ => (' ', '>'),
    };
    let fill = spec.fill.unwrap_or(if spec.zero { '0' } else { default_fill });
    let align = spec.align.unwrap_or(default_align);
    let sign = spec.sign.unwrap_or('-');
    if class == Class::Text {
        if let Some(kind) = kind.filter(|one| *one != 's') {
            return Err(unknown(kind));
        }
        if spec.sign.is_some() {
            return Err("Sign not allowed in string format specifier".into());
        }
        if spec.alternate {
            return Err("Alternate form (#) not allowed in string format specifier".into());
        }
        if spec.zeroless {
            return Err("Negative zero coercion (z) not allowed in format specifier".into());
        }
        if align == '=' {
            return Err("'=' alignment not allowed in string format specifier".into());
        }
        if let Some(grouping) = spec.grouping {
            return Err(format!("Cannot specify '{grouping}' with 's'."));
        }
        return Ok(Layout::Text {
            fill,
            align,
            width: spec.width,
            precision: spec.precision.map_or(-1, i32::from),
        });
    }
    let integral = matches!(kind, Some('b' | 'c' | 'd' | 'o' | 'x' | 'X'))
        || (kind.is_none() || kind == Some('n')) && class == Class::Integer;
    if kind == Some('s') || integral && class == Class::Float {
        return Err(unknown(kind.unwrap_or('d')));
    }
    if integral {
        if spec.precision.is_some() {
            return Err("Precision not allowed in integer format specifier".into());
        }
        if spec.zeroless {
            return Err("Negative zero coercion (z) not allowed in integer format specifier".into());
        }
        if kind == Some('c') && spec.sign.is_some() {
            return Err("Sign not allowed with integer format specifier 'c'".into());
        }
        if kind == Some('c') && spec.alternate {
            return Err("Alternate form (#) not allowed with integer format specifier 'c'".into());
        }
    }
    if let (Some(','), Some(kind @ ('b' | 'o' | 'x' | 'X' | 'c' | 'n'))) = (spec.grouping, kind) {
        return Err(format!("Cannot specify ',' with '{kind}'."));
    }
    if spec.grouping == Some('_') && matches!(kind, Some('c' | 'n')) {
        return Err(format!("Cannot specify '_' with '{}'.", kind.unwrap_or('n')));
    }
    let kind = match kind {
        None | Some('n') if class == Class::Integer => "d".to_owned(),
        None if spec.precision.is_none() => {
            return Ok(Layout::Plain {
                sign,
                fill,
                align,
                width: spec.width,
                separator,
            });
        }
        None => String::new(),
        Some('n') => "g".to_owned(),
        Some(kind) => kind.to_string(),
    };
    Ok(Layout::Format {
        precision: spec.precision.unwrap_or(6),
        kind,
        sign,
        alternate: spec.alternate,
        zeroless: spec.zeroless,
        fill,
        align,
        width: spec.width,
        separator,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_field() {
        let spec = parse("*^+z#012_.3f").expect("parses");
        assert_eq!(
            spec,
            Spec {
                fill: Some('*'),
                align: Some('^'),
                sign: Some('+'),
                zeroless: true,
                alternate: true,
                zero: true,
                width: 12,
                grouping: Some('_'),
                precision: Some(3),
                kind: Some('f'),
            }
        );
        assert_eq!(parse("<").unwrap().align, Some('<'));
        assert_eq!(parse(">>").unwrap().fill, Some('>'));
    }

    #[test]
    fn rejects_what_python_rejects() {
        let error = |text: &str, class| parse(text).and_then(|spec| layout(&spec, class)).unwrap_err();
        assert_eq!(error("q", Class::Integer), "Invalid format specifier 'q'");
        assert_eq!(error(".", Class::Float), "Format specifier missing precision");
        assert_eq!(error(",_", Class::Integer), "Cannot specify both ',' and '_'.");
        assert_eq!(error(".2d", Class::Integer), "Precision not allowed in integer format specifier");
        assert_eq!(error("d", Class::Float), "Unknown format code 'd' for object of type 'float'");
        assert_eq!(error("f", Class::Text), "Unknown format code 'f' for object of type 'str'");
        assert_eq!(error("+", Class::Text), "Sign not allowed in string format specifier");
        assert_eq!(error("=5", Class::Text), "'=' alignment not allowed in string format specifier");
        assert_eq!(error(",x", Class::Integer), "Cannot specify ',' with 'x'.");
    }
}
