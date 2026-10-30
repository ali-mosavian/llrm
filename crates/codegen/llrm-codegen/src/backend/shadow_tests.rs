use crate::abi::qb::HirAbi;
use crate::backend::assemble;
use crate::backend::cpu::ProfileOrName;
use crate::backend::shadow::{self, Selector};

const TEXT: &str =
    "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-i32:16-i64:16-f80:16-n8:16:32\"
define i16 @f(i16 %n) addrspace(1) {
entry:
  %r = add i16 %n, 1
  ret i16 %r
}
";

fn qb() -> HirAbi {
    HirAbi {
        runtime: crate::hir::model::RuntimeProfile::Qb45,
        objects: Default::default(),
        preserved: Default::default(),
        stack_check: None,
        hosted: false,
    }
}

/// A copy from a near pointer to a huge one: the DAG path refuses it (the
/// stepped routine takes two far pointers) while the old selector compiles it.
const HUGE_COPY: &str =
    "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-i32:16-i64:16-f80:16-n8:16:32\"
declare void @llvm.memcpy.p3.p0.i16(ptr addrspace(3), ptr, i16, i1)
define void @f(ptr addrspace(3) %a, ptr %b) addrspace(1) {
entry:
  call void @llvm.memcpy.p3.p0.i16(ptr addrspace(3) %a, ptr %b, i16 64, i1 false)
  ret void
}
";

fn assembled_text(
    text: &str,
    selector: Selector,
) -> Result<crate::backend::masm::Module, String> {
    assemble::assembled_by(
        &llrm_mir::parse::module(text).expect("parses"),
        &qb(),
        "T_TEXT",
        ProfileOrName::Name("486"),
        &crate::backend::target::BASIC,
        crate::backend::isel::m16(),
        selector,
        &llrm_x86_m16::M16,
        &[],
    )
}

/// `--isel=dag` produced the old selector's code while the DAG path did not
/// exist: a shadow report of old against "dag" then read zero differences. It
/// is refused when a function is one the DAG cannot select.
#[test]
fn test_the_dag_selector_refuses_a_function_it_cannot_select() {
    assert!(assembled_text(HUGE_COPY, Selector::Old).is_ok());
    let error = assembled_text(HUGE_COPY, Selector::Dag).err().expect("refused");
    assert!(error.contains("@f: --isel=dag"), "{error}");
}

#[test]
fn test_the_isel_row_names_the_selector_and_counts_the_body() {
    let module = llrm_mir::parse::module(TEXT).expect("parses");
    let body = crate::backend::isel::selected(
        &module,
        "f",
        &qb(),
        &mut crate::backend::constpool::Pool::new(0),
        crate::backend::cpu::profile("486").expect("a target"),
        &crate::backend::target::BASIC,
        crate::backend::isel::m16(),
        &llrm_x86_m16::M16,
        false,
        0,
    )
    .expect("selects")
    .body;
    let row = shadow::row(&body, Selector::Old);
    assert!(row.starts_with(&format!("f selector=old instructions={} copies=", body.insns().len())), "{row}");
    assert!(!row.contains("executed=-"), "{row}");
}

#[test]
fn test_isel_takes_old_and_dag_and_refuses_the_rest() {
    assert_eq!(Selector::parse("dag").unwrap(), Selector::Dag);
    assert_eq!(Selector::parse("old").unwrap(), Selector::Old);
    assert!(Selector::parse("fast").is_err());
}

/// A run that reports who selected proves it with `LLRM_ISEL_REPORT`: one
/// line per function, `old` or `dag`. 445 of 449 programs "passed" under
/// `--isel=dag` while the old selector compiled them.
#[test]
fn test_the_report_names_the_selector_that_selected_each_function() {
    let path = std::env::temp_dir().join(format!("isel-report-{}.txt", std::process::id()));
    let _ = std::fs::remove_file(&path);
    shadow::recorded_to(&path, "f", Selector::Old);
    shadow::recorded_to(&path, "g", Selector::Dag);
    assert_eq!(std::fs::read_to_string(&path).expect("a report"), "f old\ng dag\n");
    let _ = std::fs::remove_file(&path);
}

/// Parallel compiles append to one report: a line is one write, or two
/// processes tear each other's. A run's selection count then read a one-word
/// line and the whole `run_tests` launch died parsing it.
#[test]
fn test_report_lines_from_many_writers_stay_whole() {
    let path = std::env::temp_dir().join(format!("isel-report-many-{}.txt", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let writers: Vec<_> = (0..8)
        .map(|writer| {
            let path = path.clone();
            std::thread::spawn(move || {
                for at in 0..300 {
                    shadow::recorded_to(&path, &format!("function_{writer}_{at}"), Selector::Dag);
                }
            })
        })
        .collect();
    for writer in writers {
        writer.join().unwrap();
    }
    let report = std::fs::read_to_string(&path).expect("a report");
    assert_eq!(report.lines().count(), 8 * 300);
    assert!(report.lines().all(|line| line.starts_with("function_") && line.ends_with(" dag")), "a torn line");
    let _ = std::fs::remove_file(&path);
}

/// `LLRM_ISEL_ONLY` limits the DAG path to the named functions, so a wrong
/// result bisects to one: unset, every function; set, only those.
#[test]
fn test_isel_only_lists_the_functions_the_dag_path_selects() {
    assert!(shadow::only_listed(None, "f"));
    assert!(shadow::only_listed(Some("f,g"), "g"));
    assert!(!shadow::only_listed(Some("f,g"), "h"));
    assert!(!shadow::only_listed(Some(""), "f"));
}

const DIVIDES: &str =
    "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-i32:16-i64:16-f80:16-n8:16:32\"
define i64 @f(i64 %a, i64 %b) addrspace(1) {
entry:
  %q = sdiv i64 %a, %b
  %r = sdiv i64 %q, %b
  ret i64 %r
}
define i64 @g(i64 %a, i64 %b) addrspace(1) {
entry:
  %q = sdiv i64 %a, %b
  ret i64 %q
}
";

/// The DAG route pasted the i64 divide routine (110 bytes) at every divide of
/// every function: `stft_fixed.nib` at m16 -Os held eleven copies, 1,212
/// bytes, where the old selector's `idiv` was 20 per site. The routine is one
/// procedure of the module and each divide a call to it.
#[test]
fn test_the_dag_route_holds_the_i64_divide_routine_once_per_module() {
    let module = assembled_text(DIVIDES, Selector::Dag).expect("selects");
    let held: Vec<&str> =
        module.procedures.iter().map(|p| p.name.as_str()).filter(|name| name.starts_with("$div64_")).collect();
    assert_eq!(held, ["$div64_s"], "one routine for three divides");
    let calls = module.procedures.iter().filter(|p| !p.name.starts_with('$')).flat_map(|p| p.callees.values());
    let (shared, pasted): (Vec<_>, Vec<_>) =
        calls.filter(|c| c.name.contains("div64_")).partition(|c| c.code.is_empty());
    assert_eq!((shared.len(), pasted.len()), (3, 0), "every divide calls it");
}
