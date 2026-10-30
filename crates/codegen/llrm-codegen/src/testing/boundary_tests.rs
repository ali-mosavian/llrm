use super::boundary::{Byte, procedures};

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
    let one = run(
        "push bp\nmov bp,sp\nsub sp,8\nfld dword ptr [bp+6]\nfstp qword ptr [bp-8]\nfld qword ptr [bp-8]\nfstp dword ptr _x\nleave\nret",
    );
    assert_eq!(one.globals["_x"], (4..8).map(Byte::Incoming).collect::<Vec<_>>());
}

/// The caller's `add sp` after a call was applied on top of a reset to the
/// frame, so every later argument landed above the frame.
#[test]
fn test_a_call_and_its_cleanup_leave_the_frame_where_it_was() {
    let one = run(
        "push bp\nmov bp,sp\npush 1\ncall far ptr _g\nadd sp,2\npush 2\npush 3\ncall far ptr _h\nadd sp,4\npop bp\nret",
    );
    let stacks: Vec<_> = one.calls.iter().map(|call| (call.stack.len(), call.popped)).collect();
    assert_eq!(stacks, [(2, 2), (4, 4)]);
}

#[test]
fn test_a_struct_result_pointer_is_written_through() {
    let one = run(
        "push bp\nmov bp,sp\nles bx,dword ptr [bp+6]\nmov ax,word ptr DGROUP:_in\nmov word ptr es:[bx],ax\nmov dx,word ptr [bp+8]\nmov ax,word ptr [bp+6]\npop bp\nret",
    );
    let (_, written) = one.through.iter().next().unwrap();
    assert_eq!(written[..2], [Byte::Global("_in".into(), 0), Byte::Global("_in".into(), 1)]);
    assert_eq!(one.register("dxax"), (4..8).map(Byte::Incoming).collect::<Vec<_>>());
}

/// A BC /A listing of one SUB, its code lines as `**` rows.
fn bc_listing(
    compiler: &str,
    code: &[&str],
) -> String {
    let rows: String = code.iter().map(|one| format!(" 0000    **                  {one}\n")).collect();
    format!(
        "Offset  Data    Source Line      Microsoft (R) {compiler}\n 0030   0006    sub f (x as integer)\n 0030    **        F:        mov     cx,0000h\n{rows}"
    )
}

/// BC writes -208 as 0FF30h[bp]; read as +65328 it put callAll's
/// temporaries above the frame and every value pushed from them read as
/// an argument.
#[test]
fn test_a_bc_displacement_is_sixteen_bits_signed() {
    let text = bc_listing(
        "Visual Basic",
        &[
            "call    B$ENRA",
            "mov     word ptr [bp-208],1234h",
            "push    0FF30h[bp]",
            "call    G",
            "call    B$EXSA",
            "ret     0002h",
        ],
    );
    let one = super::boundary::procedures(&text).remove("F").unwrap();
    assert_eq!(one.calls[0].stack, [Byte::Const(0x34), Byte::Const(0x12)]);
}

/// QB 4.5 and PDS list a pool constant in memory order, VBDOS as its value;
/// read one way for all, QB 4.5's SINGLE 1.5 read as 0000C03F.
#[test]
fn test_each_compiler_spells_its_constants_its_own_way() {
    for (compiler, constant) in [("Visual Basic", "<3FC00000>"), ("QuickBASIC Compiler Version 4.50", "<0000C03F>")] {
        let text = bc_listing(
            compiler,
            &[
                "call    B$ENRA",
                &format!("push    {constant}"),
                &format!("push    {constant}"),
                "call    G",
                "call    B$EXSA",
                "ret     0002h",
            ],
        );
        let one = super::boundary::procedures(&text).remove("F").unwrap();
        assert_eq!(one.calls[0].stack, [0x00, 0x00, 0xC0, 0x3F].map(Byte::Const), "{compiler}");
    }
}
