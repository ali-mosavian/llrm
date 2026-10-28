//! A pointer made from a base and a scaled index is one `lea`.

mod common;

fn listing(
    source: &str,
    flags: &[&str],
) -> String {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(directory.join("a.c"), source).unwrap();
    let output =
        common::cc::llrm_c().current_dir(directory).args(flags).args(["-S", "-o", "a.s", "a.c"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    std::fs::read_to_string(directory.join("a.s")).unwrap()
}

fn body<'a>(
    text: &'a str,
    name: &str,
) -> Vec<&'a str> {
    let start = format!("{name} proc near");
    let end = format!("{name} endp");
    text.lines()
        .skip_while(|line| *line != start)
        .skip(1)
        .take_while(|line| *line != end)
        .map(str::trim)
        .filter(|line| !line.ends_with(':'))
        .collect()
}

/// `&p[a]` as a value was `shl ecx, 2` and `add eax, ecx`, as bintree's insert
/// loop paid three instructions for a pointer gcc makes with one `lea` (#834).
/// A load or a store through it still takes the address mode itself.
///
/// With the arguments on the stack (`-mabi=sysv`) the pin folds the base's load
/// into the add and makes the bare pointer as `shl` and `add` (three
/// instructions); a pointer the function also stores and loads through is one
/// `lea` and an indexed load.
#[test]
fn a_pointer_to_an_element_is_one_lea() {
    let text = listing(
        "int *g(int *p, int a) { return &p[a]; }\nint h(int *p, int a, int *s) { int *q = &p[a]; *s = (int)q; return *q; }\n",
        &["-m32", "-mabi=sysv", "-O2", "-march=i486"],
    );
    let g = body(&text, "_g");
    assert_eq!(g.len(), 4, "{text}");
    assert!(g.iter().any(|line| line.starts_with("add eax, dword ptr [esp+4]")), "{text}");
    let h = body(&text, "_h");
    assert!(h.iter().any(|line| line.starts_with("lea ") && line.ends_with("*4]")), "{text}");
    assert!(h.iter().any(|line| line.starts_with("mov eax, dword ptr [") && line.ends_with("*4]")), "{text}");
    assert!(!h.iter().any(|line| line.starts_with("shl ")), "{text}");
}
