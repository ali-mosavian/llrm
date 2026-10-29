//! The C front end lays types out as the target description says (`[c]`), not as its triple's default does: sizeof of
//! each type through the row's own clang command, against the description plus the row's libc. Needs the clangs
//! (`LLRM_PIN_CLANG`, `LLRM_CLANG_M16`) and the Open Watcom headers (`LLRM_OW`).

use std::process::Command;

const PROBE: &str = "typedef __SIZE_TYPE__ size_t; typedef __PTRDIFF_TYPE__ ptrdiff_t; typedef __WCHAR_TYPE__ wchar_t;
enum E { A };
int s_int = sizeof(int), s_long = sizeof(long), s_long_long = sizeof(long long), s_pointer = sizeof(void *),
    s_function_pointer = sizeof(void (*)(void)), s_size_t = sizeof(size_t), s_ptrdiff_t = sizeof(ptrdiff_t),
    s_wchar_t = sizeof(wchar_t), s_double = sizeof(double), s_long_double = sizeof(long double), s_enum = sizeof(enum E);
";

fn sizes(ir: &str) -> Vec<(String, u32)> {
    ir.lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("@s_")?;
            let (name, tail) = rest.split_once(" = ")?;
            let mut words = tail.split("global ").nth(1)?.split_whitespace();
            let (_integer_type, value) = (words.next()?, words.next()?.trim_end_matches(','));
            Some((name.to_owned(), value.parse().ok()?))
        })
        .collect()
}

#[test]
fn test_every_row_lays_types_out_as_its_description_says() {
    let table = llrm_cc::targets(include_str!("../targets.toml")).unwrap();
    let dir = std::env::temp_dir().join(format!("llrm-cc-cabi-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("p.c"), PROBE).unwrap();
    let mut checked = 0;
    for (name, row) in &table {
        let Some(want) = &row.c_abi else { continue };
        let variable = row.clang_env.as_deref().unwrap_or("LLRM_PIN_CLANG");
        let Some(clang) = std::env::var_os(variable).filter(|one| !one.is_empty()).or_else(|| std::env::var_os("LLRM_PIN_CLANG")) else {
            eprintln!("{name}: skipped, {variable} is not set");
            continue;
        };
        if row.clang.iter().any(|word| word.contains("{env:")) && std::env::var_os("LLRM_OW").is_none() {
            eprintln!("{name}: skipped, LLRM_OW is not set");
            continue;
        }
        if row.clang.iter().any(|word| word.contains("ARM64_SYSROOT")) {
            continue;
        }
        let out = dir.join(format!("{name}.ll"));
        let run = Command::new(&clang)
            .args(&row.clang)
            .args(["-S", "-emit-llvm", "-o"])
            .arg(&out)
            .arg(dir.join("p.c"))
            .output()
            .unwrap_or_else(|error| panic!("{name}: {} {error}", clang.to_string_lossy()));
        assert!(run.status.success(), "{name}: {}", String::from_utf8_lossy(&run.stderr));
        let got = sizes(&std::fs::read_to_string(&out).unwrap());
        let expected = [
            ("int", want.int),
            ("long", want.long),
            ("long_long", want.long_long),
            ("pointer", want.pointer),
            ("function_pointer", want.function_pointer),
            ("size_t", want.size_t),
            ("ptrdiff_t", want.ptrdiff_t),
            ("wchar_t", want.wchar_t),
            ("double", want.double),
            ("long_double", want.long_double),
            ("enum", want.enumeration),
        ];
        for (field, bytes) in expected {
            let found = got.iter().find(|(one, _)| one == field).map(|one| one.1);
            assert_eq!(found, Some(bytes), "{name}: sizeof {field} (description {bytes}): {got:?}");
        }
        checked += 1;
    }
    let _ = std::fs::remove_dir_all(&dir);
    eprintln!("{checked} rows checked");
}
