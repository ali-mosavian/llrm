use super::boundary::{procedures, Byte};

fn run(body: &str) -> super::boundary::Procedure {
    procedures(&format!("f proc far\n{body}\nf endp\n")).remove("f").unwrap()
}

/// `fxch st(2)` was read as a register named `st(2)` and swapped st(1):
/// p_mixed's double and long double arguments looked exchanged.
#[test]
fn test_fxch_names_the_register_it_swaps_with() {
    let one = run("fld qword ptr _a\nfld qword ptr _b\nfld qword ptr _c\nfxch st(2)\nfstp qword ptr _x\nret");
    assert_eq!(one.globals["_x"][0], Byte::Global("_a".into(), 0));
}

/// A float spilled through a wider cell and stored back at its own width
/// is the same value: p_mixed's float argument read as unknown.
#[test]
fn test_a_float_keeps_its_value_through_a_wider_cell() {
    let one = run("push bp\nmov bp,sp\nsub sp,8\nfld dword ptr [bp+6]\nfstp qword ptr [bp-8]\nfld qword ptr [bp-8]\nfstp dword ptr _x\nleave\nret");
    assert_eq!(one.globals["_x"], (4..8).map(Byte::Incoming).collect::<Vec<_>>());
}

/// The caller's `add sp` after a call was applied on top of a reset to the
/// frame, so every later argument landed above the frame.
#[test]
fn test_a_call_and_its_cleanup_leave_the_frame_where_it_was() {
    let one = run("push bp\nmov bp,sp\npush 1\ncall far ptr _g\nadd sp,2\npush 2\npush 3\ncall far ptr _h\nadd sp,4\npop bp\nret");
    let stacks: Vec<_> = one.calls.iter().map(|call| (call.stack.len(), call.popped)).collect();
    assert_eq!(stacks, [(2, 2), (4, 4)]);
}

#[test]
fn test_a_struct_result_pointer_is_written_through() {
    let one = run("push bp\nmov bp,sp\nles bx,dword ptr [bp+6]\nmov ax,word ptr DGROUP:_in\nmov word ptr es:[bx],ax\nmov dx,word ptr [bp+8]\nmov ax,word ptr [bp+6]\npop bp\nret");
    let (_, written) = one.through.iter().next().unwrap();
    assert_eq!(written[..2], [Byte::Global("_in".into(), 0), Byte::Global("_in".into(), 1)]);
    assert_eq!(one.register("dxax"), (4..8).map(Byte::Incoming).collect::<Vec<_>>());
}
