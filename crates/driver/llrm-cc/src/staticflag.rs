//! The code model of clang's IR. Clang states position independence with the
//! "PIC Level" module flag and, at `-fno-pic` without `-g` or unwind tables,
//! emits no module flags at all; `Module::position_independent` reads a module
//! without flags as position independent (hand-written IR), so the code came
//! out PIE: `lea r, [rip + sym]` and GOT loads where `-fno-pic` asks for
//! absolute addresses (`-fno-pic` copy.c -O2 had 2 `GOTPCREL`s, none with
//! `-g`).

/// `text`, clang's module, with a flag that is not "PIC Level" when it has no
/// module flags: that is static code.
pub fn stated(text: &str) -> String {
    if text.contains("!llvm.module.flags") {
        return text.to_owned();
    }
    let next = text
        .lines()
        .filter_map(|line| line.strip_prefix('!')?.split_once(' ')?.0.parse::<u32>().ok())
        .max()
        .map_or(0, |last| last + 1);
    let mut out = text.to_owned();
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&format!("!llvm.module.flags = !{{!{next}}}\n!{next} = !{{i32 8, !\"llrm.static\", i32 1}}\n"));
    out
}

#[cfg(test)]
mod tests {
    use super::stated;

    #[test]
    fn a_module_without_flags_gets_one_that_is_not_pic_level() {
        let out = stated("define void @f() {\n  ret void\n}\n!3 = !{}\n");
        assert!(
            out.contains("!llvm.module.flags = !{!4}") && out.contains("!4 = !{i32 8, !\"llrm.static\", i32 1}"),
            "{out}"
        );
    }

    #[test]
    fn a_module_with_flags_is_left_as_clang_wrote_it() {
        let text = "!llvm.module.flags = !{!0}\n!0 = !{i32 8, !\"PIC Level\", i32 2}\n";
        assert_eq!(stated(text), text);
    }
}
