"""The loop corpus's own instruments: tools/loops."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools" / "loops"))

from tools import innerloops  # noqa: E402
from tests.test_innerloops import _object  # noqa: E402

import quality  # noqa: E402


def _facts(code: bytes):
    return [quality.facts(one) for one in innerloops.loops(_object(code, "F"), calls=True)]


def test_a_counter_carried_through_a_copy_is_one_induction_variable():
    """bubble's j lives in si, is copied to cx, stepped there and copied
    back: no instruction adds a constant to the register it reads, so the
    count was 0."""
    code = bytes.fromhex(
        "8b00"  # loop: mov ax,[bx+si]
        "89f1"  # mov cx,si
        "41"  # inc cx
        "89ce"  # mov si,cx
        "39ee"  # cmp si,bp
        "7cf5"  # jl loop
        "cb"  # retf
    )
    assert [(one.ivs, one.iv_registers) for one in _facts(code)] == [(1, ["si"])]


def test_a_counter_feeding_only_lea_is_an_induction_variable():
    """strcmp's counter reaches memory through `lea si,[ecx+ecx]`; lea
    touches no memory, so the counter was not seen as an address."""
    code = bytes.fromhex(
        "41"  # loop: inc cx
        "678d3409"  # lea si,[ecx+ecx]
        "8b14"  # mov dx,[si]
        "85d2"  # test dx,dx
        "75f5"  # jne loop
        "cb"  # retf
    )
    assert [one.ivs for one in _facts(code)] == [1]


import dos  # noqa: E402
import expect  # noqa: E402
import subprocess  # noqa: E402
from spec import Array, Case, Input, Fill, I16, I32, For, Assign, Return, Load, Bin, Cast, c, v, add  # noqa: E402
from cases import concurrent  # noqa: E402


def test_a_stopped_program_s_reports_come_from_what_it_wrote(tmp_path):
    """DOS sets a file's size when it is closed; a program stopped mid-run
    leaves its output file empty, so a crash read as crashing before its
    first report and was charged to the wrong case."""
    (tmp_path / "P.EXE").write_bytes(b"MZ")
    (tmp_path / "P.TXT").write_bytes(b"")
    events = tmp_path / "events.txt"
    events.write_text(
        '{"ev":"end","reason":"exit"}\n'
        '{"ev":"out","handle":1,"file":"P.TXT","text":"5\\r\\n-7\\r\\n"}\n'
        '{"ev":"end","reason":"crash","ms":3}\n'
    )
    got = dos.collect([dos.Job("p", "exe", tmp_path / "P.EXE")], tmp_path, events)["p"]
    assert isinstance(got, dos.Stopped) and got.partial == [5, -7]


def test_the_c_start_up_gives_dos_a_stack_outside_the_code(tmp_path):
    """With no STACK segment DOS started the program with SS:SP inside its
    code; a timer interrupt before the start-up switched stacks wrote six
    bytes into a procedure, and the program restarted itself forever."""
    bin_ = Path(__file__).resolve().parents[1] / "target" / "release"
    here = Path(__file__).resolve().parents[1] / "tools" / "loops" / "runtime"
    (tmp_path / "m.asm").write_text(".model medium\n.code\npublic _main\n_main proc far\n    ret\n_main endp\nend\n")
    for source, obj in ((here / "crt.asm", "crt.obj"), (tmp_path / "m.asm", "m.obj")):
        subprocess.run([bin_ / "jwasm", "-q", "-c", "-Cp", "-Zg", "-omf", f"-Fo{tmp_path / obj}", source], check=True)
    subprocess.run([bin_ / "jwlink", "option", "quiet", "format", "dos", "name", tmp_path / "p.exe",
                    "file", tmp_path / "crt.obj", "file", tmp_path / "m.obj"], check=True, capture_output=True)
    exe = (tmp_path / "p.exe").read_bytes()
    # the code is at the load image's start; the stack must be elsewhere
    assert int.from_bytes(exe[14:16], "little") > 0


def _one_loop(body, arrays, params=(("n", I16),), locals_=(("i", I16), ("s", I32))):
    return Case("t", "test", params, arrays, locals_, body, I32, (Input((1,), tuple((a.name, Fill(1)) for a in arrays)),))


def test_an_invariant_inside_an_address_takes_no_register():
    """matmul's inner loop reads a[i][k]: i is folded into the row's base
    before the loop, but was counted as a live invariant, so the loop was
    judged past the registers and got no bound at all."""
    a, b = Array("a", I16, (16, 16)), Array("b", I16, (16, 16))
    body = (For("k", c(0), "<", v("n"), c(1), (
        Assign(v("s"), add(v("s"), Cast(Load("a", (v("i"), v("k"))), I32))),
        Assign(v("s"), add(v("s"), Cast(Load("b", (v("j"), v("k"))), I32))),
    )), Return(v("s")))
    case = _one_loop(body, (a, b), params=(("n", I16), ("i", I16), ("j", I16)), locals_=(("k", I16), ("s", I32)))
    [want] = expect.want(case, "c")
    assert want.fits and want.ivs == 1


def test_an_access_that_is_not_affine_is_not_the_reference_shape():
    """histogram's h[a[i] & 15] was skipped as not affine and the loop still
    claimed the one-counter shape."""
    a, h = Array("a", I16, (300,)), Array("h", I16, (16,))
    slot = Bin("&", Load("a", (v("i"),)), c(15))
    body = (For("i", c(0), "<", v("n"), c(1), (Assign(Load("h", (slot,)), add(Load("h", (slot,)), c(1))),)),
            Return(Cast(v("i"), I32)))
    [want] = expect.want(_one_loop(body, (a, h)), "c")
    assert not want.shape


def test_the_concurrent_hand_count_agrees_with_the_spec_derivation():
    """Two derivations of the same bound, one from the generator's walks and
    one from the built loop, must agree wherever the loop fits."""
    disagree = []
    for shape in concurrent.shapes():
        if shape.form != "index" or shape.use != "sum" or shape.call or shape.trip != "n" or shape.step != 1:
            continue
        case = concurrent.build(shape)
        for lang in ("c", "bas", "nib"):
            [want] = expect.want(case, lang)
            if want.fits and want.ivs != concurrent.hand_ivs(shape, lang):
                disagree.append((case.name, lang, want.ivs, concurrent.hand_ivs(shape, lang)))
    assert disagree == []


def test_a_return_while_bp_holds_a_base_is_found():
    """bp released as a seventh register must be the frame again at every
    exit; an early exit that skips `pop bp` returns with a wrecked frame."""
    code = bytes.fromhex(
        "55"  # push bp
        "8bee"  # mov bp,si        (bp now a base)
        "85c0"  # test ax,ax
        "7401"  # je out
        "5d"  # pop bp
        "cb"  # out: retf          (reached from je with bp still si)
    )
    text = innerloops.procedures(_object(code, "F"))["F"]
    assert quality.bp_problems(text) != []


import oracle  # noqa: E402
from cases.concurrent import Walk, Shape  # noqa: E402


def _runs(shape):
    case = concurrent.build(shape)
    return oracle.evaluate(case, "c")


def test_a_walk_may_share_the_array_of_a_walk_that_shares_one():
    """The fuzzer drew walk 2 onto walk 1, itself on walk 0: the array was
    looked up under walk 1's name, which owns none, and the build crashed."""
    walks = (Walk(I16), Walk(I16, ("off", 1), same_as=0), Walk(I16, ("off", 2), same_as=1))
    assert all(isinstance(one, list) for one in _runs(Shape(walks)))


def test_the_end_pointer_is_set_inside_an_outer_loop():
    """With an outer loop the end pointer's index reads its counter, but it
    was computed before that loop: `k is read before it is set`."""
    assert all(isinstance(one, list) for one in _runs(Shape((Walk(I16),), form="end", outer=True)))


def test_continue_in_a_do_while_still_steps():
    """A do-while steps its counter at the body's end; a `continue` there
    skipped the step and the loop never ended."""
    shape = Shape((Walk(I16),), form="do", extras=(("exit", "continue"),))
    assert all(isinstance(one, list) for one in _runs(shape))
