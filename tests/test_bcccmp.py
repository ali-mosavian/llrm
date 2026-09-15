"""The bcc comparison counts what both compilers wrote, and only that."""

from tools import bcccmp


def test_function_begins_at_its_definition_not_its_prototype():
    """Starts taken from the line cued before each declaration put sys_rdtsc_hz
    in no region, and 31 bcc regions ran on into the next function."""
    source = "static short f( void );\n\nshort g( void ) { return f(); }\n\nstatic short f( void )\n{\n    return 1;\n}\n"
    assert bcccmp.definitions(source, ["f", "g"]) == {"f": 5, "g": 3}


def test_function_ends_before_its_switch_table():
    """config_load decoded its jump table as `add bh,[bp+di+2]; lahf; (bad)`."""
    image = bytes([0x2E, 0xFF, 0xA7, 0x06, 0x00, 0xCB, 0x00, 0x00, 0x05, 0x00])
    assert bcccmp.extents(image, [(0, "f")], {}) == {"f": (0, 6)}


def test_a_pattern_does_not_run_across_a_label():
    """Dropping a jmp joined sys_parse_args' `add ax,2` to a store behind label
    L1_955, and the load-op-store count rose for a site no peephole could fuse."""
    split = ["mov ax, word ptr [bp-76]", "add ax, 2", "L1_955:", "mov word ptr [bp-76], ax"]
    assert bcccmp.counted(split)["slot load, op, store back"] == 0
    assert bcccmp.counted([one for one in split if not one.endswith(":")])["slot load, op, store back"] == 1


def test_loop_entry_compare_is_seen_past_its_header_label():
    """Keeping labels in hid every `xor cx,cx` / `L0_20:` / `cmp cx,48`: the row read 0 for 60 sites."""
    entry = ["xor cx, cx", "L0_20:", "cmp cx, 48", "jge L0_79"]
    assert bcccmp.counted(entry)["constant compare at loop entry"] == 1


def test_fwait_is_not_an_instruction_choice():
    """-f87 writes `wait` before x87 instructions: 1,727 of them counted as bcc's."""
    assert bcccmp.instructions(["0000 wait", "0001 fld st(1)"], listing=True) == ["fld st(1)"]


def test_inline_bytes_are_decoded():
    """ftol_short's two `db` lines stood for eleven instructions and counted as two."""
    assert bcccmp.instructions(["db 066h,053h", "db 00fh,031h"], listing=False) == ["push ebx", "rdtsc"]
