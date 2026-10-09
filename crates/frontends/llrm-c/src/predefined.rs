//! The macros GCC predefines that programs test and build types from (`__INT_MAX__`, `__SIZE_TYPE__`, `__BYTE_ORDER__`,
//! ...), which Open Watcom does not. One table: the sizes are the front end's own (`widths_for`), the limits and the
//! types follow from them, and the byte order is the target's (x86 is little endian). A clang front end would define
//! them itself; this is the small route until then.

use crate::raise_hir::widths_for;

/// The `#define`s of a unit, flat (the 386 tree) or not.
pub fn header(flat: bool) -> String {
    let size = |type_: &str| i64::from(widths_for(flat, type_).expect("a front-end type has a width"));
    let (int, short, long, long_long, pointer) =
        (size("TY_INTEGER"), size("TY_INT_2"), size("TY_INT_4"), size("TY_INT_8"), size("TY_NEAR_POINTER"));
    let mut out = Vec::new();
    let mut define = |name: &str, value: String| out.push(format!("#define {name} {value}"));
    define("__CHAR_BIT__", "8".to_owned());
    for (name, bytes) in [
        ("SHORT", short),
        ("INT", int),
        ("LONG", long),
        ("LONG_LONG", long_long),
        ("POINTER", pointer),
        ("FLOAT", 4),
        ("DOUBLE", 8),
        ("LONG_DOUBLE", 10),
    ] {
        define(&format!("__SIZEOF_{name}__"), bytes.to_string());
    }
    let signed_max = |bytes: i64, suffix: &str| format!("{}{suffix}", (1_u128 << (bytes * 8 - 1)) - 1);
    define("__SCHAR_MAX__", "127".to_owned());
    define("__SHRT_MAX__", signed_max(short, ""));
    define("__INT_MAX__", signed_max(int, ""));
    define("__LONG_MAX__", signed_max(long, "L"));
    define("__LONG_LONG_MAX__", signed_max(long_long, "LL"));
    // The type each width is spelled with; Open Watcom's size_t and ptrdiff_t are the unsigned and signed int, its
    // wchar_t an unsigned short.
    let spelled = |bytes: i64, signed: bool| {
        let base = match bytes {
            1 => "char",
            b if b == short => "short",
            b if b == int => "int",
            b if b == long => "long",
            _ => "long long",
        };
        match (base, signed) {
            ("char", true) => "signed char".to_owned(),
            (base, true) => base.to_owned(),
            (base, false) => format!("unsigned {base}"),
        }
    };
    // A near pointer is an int wide on both trees: the sizes agree, and Open Watcom's size_t is the unsigned int.
    assert_eq!(pointer, int, "a near pointer is as wide as an int");
    define("__SIZE_TYPE__", "unsigned int".to_owned());
    define("__PTRDIFF_TYPE__", "int".to_owned());
    define("__INTPTR_TYPE__", "int".to_owned());
    define("__UINTPTR_TYPE__", "unsigned int".to_owned());
    define("__WCHAR_TYPE__", spelled(short, false));
    define("__WCHAR_WIDTH__", (short * 8).to_string());
    for bits in [8_i64, 16, 32, 64] {
        let bytes = bits / 8;
        define(&format!("__INT{bits}_TYPE__"), spelled(bytes, true));
        define(&format!("__UINT{bits}_TYPE__"), spelled(bytes, false));
        define(&format!("__INT_LEAST{bits}_TYPE__"), spelled(bytes, true));
        define(&format!("__UINT_LEAST{bits}_TYPE__"), spelled(bytes, false));
    }
    define("__ORDER_LITTLE_ENDIAN__", "1234".to_owned());
    define("__ORDER_BIG_ENDIAN__", "4321".to_owned());
    define("__ORDER_PDP_ENDIAN__", "3412".to_owned());
    define("__BYTE_ORDER__", "__ORDER_LITTLE_ENDIAN__".to_owned());
    // IEEE single and double, and the x87's extended.
    for (name, max, min, epsilon, digits, mantissa) in [
        ("FLT", "3.40282346638528859812e+38F", "1.17549435082228750797e-38F", "1.19209289550781250000e-7F", 6, 24),
        ("DBL", "1.79769313486231570815e+308", "2.22507385850720138309e-308", "2.22044604925031308085e-16", 15, 53),
        (
            "LDBL",
            "1.18973149535723176502e+4932L",
            "3.36210314311209350626e-4932L",
            "1.08420217248550443401e-19L",
            18,
            64,
        ),
    ] {
        define(&format!("__{name}_MAX__"), max.to_owned());
        define(&format!("__{name}_MIN__"), min.to_owned());
        define(&format!("__{name}_EPSILON__"), epsilon.to_owned());
        define(&format!("__{name}_DIG__"), digits.to_string());
        define(&format!("__{name}_MANT_DIG__"), mantissa.to_string());
    }
    out.join("\n") + "\n"
}

#[cfg(test)]
mod tests {
    use super::header;

    fn value(
        text: &str,
        name: &str,
    ) -> String {
        let prefix = format!("#define {name} ");
        text.lines()
            .find_map(|line| line.strip_prefix(prefix.as_str()))
            .unwrap_or_else(|| panic!("{name} is defined"))
            .to_owned()
    }

    /// Open Watcom defines none of GCC's: 73 gcc.c-torture programs were refused on `__SIZE_TYPE__` alone and
    /// widechar-3 took the big-endian branch of an `#if __BYTE_ORDER__ == __ORDER_BIG_ENDIAN__`, both sides 0.
    #[test]
    fn test_the_macros_follow_the_targets_sizes() {
        let (flat, segmented) = (header(true), header(false));
        assert_eq!(
            (value(&flat, "__INT_MAX__"), value(&segmented, "__INT_MAX__")),
            ("2147483647".to_owned(), "32767".to_owned())
        );
        assert_eq!(
            (value(&flat, "__SIZE_TYPE__"), value(&segmented, "__SIZE_TYPE__")),
            ("unsigned int".to_owned(), "unsigned int".to_owned())
        );
        assert_eq!(
            (value(&flat, "__SIZEOF_POINTER__"), value(&segmented, "__SIZEOF_POINTER__")),
            ("4".to_owned(), "2".to_owned())
        );
        assert_eq!(value(&flat, "__LONG_LONG_MAX__"), "9223372036854775807LL");
        assert_eq!(value(&flat, "__INT32_TYPE__"), "int");
        assert_eq!(value(&segmented, "__INT32_TYPE__"), "long");
        assert_eq!(value(&flat, "__BYTE_ORDER__"), "__ORDER_LITTLE_ENDIAN__");
    }
}
