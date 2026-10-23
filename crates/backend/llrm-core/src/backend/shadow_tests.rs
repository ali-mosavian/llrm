use crate::abi::qb::HirAbi;
use crate::backend::assemble;
use crate::backend::cpu::ProfileOrName;
use crate::backend::shadow::{self, Selector};

const TEXT: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-i32:16-i64:16-n8:16:32\"
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
    }
}

/// A load through a far pointer: the DAG path still refuses an address in
/// another space.
const FAR_LOAD: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-i32:16-i64:16-n8:16:32\"
define i16 @f(ptr addrspace(1) %p) addrspace(1) {
entry:
  %v = load i16, ptr addrspace(1) %p
  ret i16 %v
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
    assert!(assembled_text(FAR_LOAD, Selector::Old).is_ok());
    let error = assembled_text(FAR_LOAD, Selector::Dag).err().expect("refused");
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
