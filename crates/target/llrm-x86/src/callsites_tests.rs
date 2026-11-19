use super::{Value, scan};

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|one| (*one).to_owned()).collect()
}

fn calls(
    code: &[u8],
    fixups: &[(usize, &str)],
) -> Vec<(usize, Option<String>, Vec<(String, Value)>)> {
    let arguments = names(&["rdi", "rsi", "rdx"]);
    let preserved = names(&["rbx", "r12"]);
    scan(code, 64, fixups, &arguments, &preserved, 1, &|_| 2)
        .calls
        .into_iter()
        .map(|one| (one.offset, one.symbol, one.parameters))
        .collect()
}

fn pair(
    register: &str,
    value: Value,
) -> (String, Value) {
    (register.to_owned(), value)
}

/// `mov esi, 7; call g` with `rdi` untouched since entry.
#[test]
fn a_constant_and_an_untouched_argument_describe_the_call() {
    let code = [0xbe, 7, 0, 0, 0, 0xe8, 0, 0, 0, 0];
    assert_eq!(
        calls(&code, &[(6, "g")]),
        [(10, Some("g".into()), vec![pair("rdi", Value::Entry), pair("rsi", Value::Constant(7))])]
    );
}

/// `mov rdi, rbx` is the saved register; `lea rsi, [r12+8]` is it plus 8.
#[test]
fn a_copy_of_a_preserved_register_is_that_register_plus_its_displacement() {
    let code = [0x48, 0x89, 0xdf, 0x49, 0x8d, 0x74, 0x24, 8, 0xe8, 0, 0, 0, 0];
    assert_eq!(
        calls(&code, &[(9, "g")]),
        [(
            13,
            Some("g".into()),
            vec![pair("rdi", Value::Register("rbx".into(), 0)), pair("rsi", Value::Register("r12".into(), 8))]
        )]
    );
}

/// A branch before the writes ends the run, so `rdi` could have come by either
/// path; and a preserved register written after its copy is no longer it.
#[test]
fn what_a_branch_or_a_later_write_could_change_is_left_out() {
    // test edi, edi; je +0; mov esi, 7; call g
    let code = [0x85, 0xff, 0x74, 0, 0xbe, 7, 0, 0, 0, 0xe8, 0, 0, 0, 0];
    assert_eq!(calls(&code, &[(10, "g")]), [(14, Some("g".into()), vec![pair("rsi", Value::Constant(7))])]);
    // mov rdi, rbx; xor ebx, ebx; call g
    let code = [0x48, 0x89, 0xdf, 0x31, 0xdb, 0xe8, 0, 0, 0, 0];
    assert_eq!(calls(&code, &[(6, "g")]), [(10, Some("g".into()), Vec::new())]);
}

/// A jump to another symbol leaves the function without a site.
#[test]
fn a_jump_to_another_symbol_is_a_tail_call() {
    let arguments = names(&["rdi"]);
    let found = scan(&[0xe9, 0, 0, 0, 0], 64, &[(1, "g")], &arguments, &[], 1, &|_| 1);
    assert!(found.tail_call && found.calls.is_empty());
}
