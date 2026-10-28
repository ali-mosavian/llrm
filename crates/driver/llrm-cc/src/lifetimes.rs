//! The pin's `llvm.lifetime.start/end` take only the pointer; MIR's still take
//! the size first (`llrm-mir` `intrinsics.rs`), so the pin's markers were calls
//! no pass recognised and the inliner's own (two-argument) markers called a
//! one-argument declaration: B_SENV of the QB runtime's `environ.c` reached
//! simplifycfg with a phi missing an arm and panicked.

/// `text` with every lifetime marker and its declaration in MIR's form. The
/// size is -1, LLVM's "unknown".
pub fn upgraded(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + text.len() / 16);
    for line in text.lines() {
        out.push_str(&upgraded_line(line));
        out.push('\n');
    }
    out
}

fn upgraded_line(line: &str) -> String {
    let Some(name) = ["@llvm.lifetime.start.p", "@llvm.lifetime.end.p"].iter().find_map(|name| line.find(name)) else {
        return line.to_owned();
    };
    let Some(open) = line[name..].find('(').map(|at| name + at + 1) else { return line.to_owned() };
    let rest = &line[open..];
    if rest.starts_with("i64") {
        return line.to_owned();
    }
    let insert = if line.trim_start().starts_with("declare") {
        "i64 immarg, "
    } else if rest.starts_with("ptr") {
        "i64 -1, "
    } else {
        return line.to_owned();
    };
    format!("{}{insert}{rest}", &line[..open])
}

#[cfg(test)]
mod tests {
    use super::upgraded;

    const PIN: &str = "target datalayout = \"e-p:64:64-i64:64-n32:64\"
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1
define void @f() {
  %1 = alloca i32, align 4
  call void @llvm.lifetime.start.p0(ptr nonnull %1) #2
  store i32 1, ptr %1, align 4
  call void @llvm.lifetime.end.p0(ptr %1) #2
  ret void
}
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nounwind }
";

    /// The pin's markers did not verify as MIR's ("Intrinsic has incorrect
    /// argument type!"), and without the check ran as unknown calls.
    #[test]
    fn the_pins_markers_verify_as_mirs() {
        let module = llrm_mir::parse::module(&upgraded(PIN)).expect("parses");
        assert_eq!(llrm_mir::verify::verify(&module), Vec::<String>::new());
        let raw = llrm_mir::parse::module(PIN).expect("parses");
        assert!(!llrm_mir::verify::verify(&raw).is_empty(), "the pin's own text is not MIR's");
    }

    #[test]
    fn a_marker_already_in_mirs_form_is_left_alone() {
        let text = "call void @llvm.lifetime.start.p0(i64 4, ptr %1)\n";
        assert_eq!(upgraded(text), text);
    }
}
