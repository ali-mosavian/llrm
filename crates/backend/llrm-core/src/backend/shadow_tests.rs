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

fn assembled(selector: Selector) -> Result<crate::backend::masm::Module, String> {
    assemble::assembled_by(
        &llrm_mir::parse::module(TEXT).expect("parses"),
        &qb(),
        "T_TEXT",
        ProfileOrName::Name("486"),
        &crate::backend::target::BASIC,
        crate::backend::isel::m16(),
        selector,
        &llrm_x86_m16::M16,
    )
}

/// `--isel=dag` produced the old selector's code while the DAG path did not
/// exist: a shadow report of old against "dag" then read zero differences. It
/// is refused until a function is selected by the DAG.
#[test]
fn test_the_dag_selector_refuses_a_function_it_cannot_select() {
    assert!(assembled(Selector::Old).is_ok());
    let error = assembled(Selector::Dag).err().expect("refused");
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
