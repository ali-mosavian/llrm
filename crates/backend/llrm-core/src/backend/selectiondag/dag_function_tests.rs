use super::dag_function::selected;

const SUM: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(i32 %a, i32 %b) {
entry:
  %s = add i32 %a, %b
  ret i32 %s
}
";

const TWO_BLOCKS: &str = "target datalayout = \"e-p:32:32-i64:32-n8:16:32\"
define i32 @f(i32 %a) {
entry:
  br label %next
next:
  ret i32 %a
}
";

fn names(text: &str) -> Result<Vec<String>, String> {
    let module = llrm_mir::parse::module(text).expect("parses");
    let done = selected(&module, "f", &llrm_x86_m32::M32).map_err(|e| e.0)?;
    Ok(done
        .body
        .insns()
        .iter()
        .map(|i| i.what.as_ref().and_then(|w| w.name.clone()).unwrap_or_default())
        .collect())
}

/// `--isel=dag` made no function: every compile with it was refused. A sum of
/// two stack arguments comes out as the instructions the pin selects.
#[test]
fn a_function_of_one_block_is_selected_to_lir() {
    assert_eq!(names(SUM).unwrap(), ["mov", "add", "mov", "ret"]);
}

/// A function of several blocks needs the branches and phis (SD403): it is a
/// refusal with its reason, which the shadow report counts.
#[test]
fn a_function_of_several_blocks_is_refused_with_its_reason() {
    let why = names(TWO_BLOCKS).unwrap_err();
    assert!(why.contains("--isel=dag: a function of several blocks"), "{why}");
}
