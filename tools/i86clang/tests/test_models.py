"""The memory-model matrix of the clang front end (tools/i86clang): five models, each over the same probes.

A model fixes the default distance of code and of data (small: near/near, medium: far/near, compact: near/far, large and huge:
far/far); `near`, `far` and `huge` on a pointer override the data default; a function pointer follows the code distance.
Expected values are what Borland C 3.1 (`bcc -m?`) and Open Watcom (`wcc -m?`) produce, read from their own output: sizes from
the objects, arithmetic and comparison from the code.

Needs the tool (tools/i86clang/build.sh), clang and llrm-llc; each test is skipped when one is missing.
"""
import os
import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(HERE.parent))
import borland  # noqa: E402

TOOL = Path(os.environ.get("I86CLANG", HERE.parent / "build" / "i86clang"))
CLANG = os.environ.get("CLANG", "/opt/homebrew/opt/llvm/bin/clang")
LLC = Path(os.environ.get("LLRM_LLC", ROOT / "target" / "release" / "llrm-llc"))

# model: (code is far, data is far)
MODELS = {"small": (False, False), "medium": (True, False), "compact": (False, True), "large": (True, True), "huge": (True, True)}


def have_tools():
    return TOOL.exists() and Path(CLANG).exists() and LLC.exists()


def ir_of(model, source):
    """`source` (Borland C) preprocessed, rewritten and compiled by i86clang for `model`, as LLVM IR text."""
    pre = subprocess.run([CLANG, "-E", "-P", "-x", "c", "--target=msp430", "-std=gnu89", str(HERE / source)],
                         capture_output=True, text=True, check=True).stdout
    with tempfile.TemporaryDirectory() as work:
        src = Path(work) / "m.c"
        src.write_text(borland.rewrite(pre))
        run = subprocess.run([str(TOOL), f"-model={model}", str(src), "--", "--target=msp430", "-std=gnu89", "-w"],
                             capture_output=True, text=True)
    if run.returncode:
        raise AssertionError(run.stderr[-2000:])
    return run.stdout


def asm_of(ir):
    """llrm-llc's MASM for `ir`, at -O2."""
    with tempfile.TemporaryDirectory() as work:
        ll, out = Path(work) / "m.ll", Path(work) / "m.s"
        ll.write_text(ir)
        run = subprocess.run([str(LLC), "-O2", "-S", "-o", str(out), str(ll)], capture_output=True, text=True)
        if run.returncode:
            raise AssertionError(run.stderr[-2000:])
        return out.read_text()


def constants(ir):
    """The `i16` globals of `ir` by name: what `sizeof` came to."""
    return {m.group(1): int(m.group(2)) for m in re.finditer(r"^@(\w+) = (?:dso_local )?(?:addrspace\(\d\) )?global i16 (\d+)", ir, re.M)}


def body(asm, name):
    """The instructions of procedure `name` (its `_` spelling), without frame setup."""
    found = re.search(rf"^_{name} proc.*?^_{name} endp", asm, re.M | re.S)
    assert found, f"no procedure {name} in\n{asm}"
    return [line.strip() for line in found.group(0).splitlines()[1:-1]
            if line.strip() and not line.startswith("L") and line.strip() not in ("push bp", "mov bp, sp", "pop bp")]


def space_of(ir, name):
    """The pointer type of global `name` (`ptr` or `ptr addrspace(N)`), the one a pointer variable is stored as."""
    return re.search(rf"^@{name} = (?:dso_local )?global (ptr(?: addrspace\(\d\))?)", ir, re.M).group(1)


@unittest.skipUnless(have_tools(), "needs tools/i86clang/build.sh, clang and a built llrm-llc")
class Models(unittest.TestCase):
    cache = {}

    @classmethod
    def compiled(cls, model, source):
        key = (model, source)
        if key not in cls.cache:
            ir = ir_of(model, source)
            cls.cache[key] = (ir, asm_of(ir))
        return cls.cache[key]

    def each(self, check):
        for model, (code_far, data_far) in MODELS.items():
            with self.subTest(model=model):
                check(model, code_far, data_far)

    def test_pointer_sizes_follow_the_distance_each_is_declared_or_defaulted_to(self):
        def check(model, code_far, data_far):
            sizes = constants(self.compiled(model, "defaults.c")[0])
            self.assertEqual(
                {"np": 4 if data_far else 2, "fp": 4, "hp": 4, "nf": 4 if code_far else 2, "int": 2, "size": 2},
                {key: sizes[f"s_{key}"] for key in ("np", "fp", "hp", "nf", "int", "size")},
            )
        self.each(check)

    def test_a_pointer_difference_is_a_long_where_data_pointers_are_far(self):
        # bcc: sizeof(np - np) is 4 in compact, large and huge, 2 below.
        self.each(lambda model, code_far, data_far: self.assertEqual(
            4 if data_far else 2, constants(self.compiled(model, "defaults.c")[0])["s_diff"]))

    def test_a_struct_is_laid_out_by_its_pointers_widths(self):
        # a data pointer, a function pointer and an int: 2 + 2 + 2 below, 4 + 2 + 2 and 2 + 4 + 2 by the distance of each.
        self.each(lambda model, code_far, data_far: self.assertEqual(
            (4 if data_far else 2) + (4 if code_far else 2) + 2, constants(self.compiled(model, "defaults.c")[0])["s_struct"]))

    def test_an_unmarked_data_pointer_is_in_the_models_space_and_far_and_huge_ones_in_their_own(self):
        def check(model, code_far, data_far):
            ir = self.compiled(model, "defaults.c")[0]
            self.assertEqual(
                ("ptr addrspace(1)" if data_far else "ptr", "ptr addrspace(1)", "ptr addrspace(3)"),
                (space_of(ir, "np"), space_of(ir, "fp"), space_of(ir, "hp")))
        self.each(check)

    def test_a_function_and_a_function_pointer_are_far_where_code_is(self):
        def check(model, code_far, data_far):
            ir = self.compiled(model, "defaults.c")[0]
            self.assertEqual("ptr addrspace(1)" if code_far else "ptr", space_of(ir, "nf"))
            self.assertEqual(code_far, bool(re.search(r"^define .* @plain\(\) addrspace\(1\)", ir, re.M)))
        self.each(check)

    def test_a_pointer_in_a_declaration_list_does_not_qualify_its_neighbours(self):
        # `char buf[4], *p, **pp, c;` once failed to compile ("automatic variable qualified with an address space"): the space
        # for `*p` was written in the specifier the whole list shares. buf, c and k stay what they were.
        def check(model, code_far, data_far):
            ir, asm = self.compiled(model, "lists.c")
            sizes = constants(ir)
            data = 4 if data_far else 2
            self.assertEqual((4, 1, 3 * data + 2), (sizes["s_buf"], sizes["s_c"], sizes["s_R"]))
            self.assertEqual("ptr addrspace(1)" if data_far else "ptr", space_of(ir, "p"))
        self.each(check)

    def test_an_explicit_function_pointer_distance_is_the_models_code_distance(self):
        # Limit: clang cannot qualify a function type with an address space, so `int (far *)(void)` in a near-code model, or
        # `int (near *)(void)` in a far-code one, is the model's own width. bcc and wcc give them the distance written.
        def check(model, code_far, data_far):
            sizes = constants(self.compiled(model, "defaults.c")[0])
            self.assertEqual((4 if code_far else 2,) * 2, (sizes["s_ff"], sizes["s_nn"]))
        self.each(check)

    def test_far_arithmetic_changes_the_offset_alone(self):
        # p + i: one add of the offset, the segment passes through; q - p: the offsets' difference.
        def check(model, code_far, data_far):
            asm = self.compiled(model, "pointers.c")[1]
            add, diff = body(asm, "far_add"), body(asm, "far_diff")
            self.assertEqual(1, sum(line.startswith("add ") for line in add), add)
            self.assertFalse([line for line in add if line.startswith(("adc", "shl", "sar"))], add)
            self.assertEqual(1, sum(line.startswith("sub ") for line in diff), diff)
        self.each(check)

    def test_far_pointers_order_by_their_offsets_and_are_equal_by_both_words(self):
        def check(model, code_far, data_far):
            asm = self.compiled(model, "pointers.c")[1]
            less, same = body(asm, "far_less"), body(asm, "far_same")
            self.assertEqual(1, sum(line.startswith("cmp ") for line in less), less)
            self.assertFalse([line for line in less if line.startswith("sbb")], less)
            self.assertEqual(2, sum(line.startswith("xor ") for line in same), same)
        self.each(check)

    def test_huge_arithmetic_carries_into_the_segment(self):
        def check(model, code_far, data_far):
            asm = self.compiled(model, "pointers.c")[1]
            add, less = body(asm, "huge_add"), body(asm, "huge_less")
            self.assertTrue([line for line in add if line.startswith("shl ")], add)
            self.assertTrue([line for line in less if line.startswith("sbb")], less)
        self.each(check)

    def test_a_huge_difference_is_llrms_ptrdiff_not_a_subtraction_of_packed_pointers(self):
        self.each(lambda model, code_far, data_far: self.assertIn(
            "@llrm.ia16.ptrdiff.i32.p3(", self.compiled(model, "pointers.c")[0]))

    def test_near_to_far_reads_the_data_segment_and_far_to_near_drops_the_segment(self):
        def check(model, code_far, data_far):
            asm = self.compiled(model, "pointers.c")[1]
            self.assertTrue([line for line in body(asm, "to_far") if "DGROUP" in line], body(asm, "to_far"))
            self.assertFalse([line for line in body(asm, "to_near") if "dx" in line or "es" in line], body(asm, "to_near"))
        self.each(check)

    def test_a_near_pointer_is_read_through_ds_and_a_far_one_through_es(self):
        def check(model, code_far, data_far):
            asm = self.compiled(model, "pointers.c")[1]
            self.assertFalse([line for line in body(asm, "near_load") if "es" in line], body(asm, "near_load"))
            self.assertTrue([line for line in body(asm, "far_load") if "es:" in line], body(asm, "far_load"))
        self.each(check)

    def test_a_function_is_named_and_cleaned_up_by_its_convention(self):
        # cdecl: `_name`, the caller pops; pascal: `NAME` in capitals, the callee pops (`ret 4`); far where code is.
        def check(model, code_far, data_far):
            asm = self.compiled(model, "conventions.c")[1]
            self.assertIn("public _c_add\n", asm)
            self.assertIn("public P_ADD\n", asm)
            ret = "retf" if code_far else "ret"
            self.assertEqual([f"{ret} 4"], [line for line in body(asm, "c_add") + body_of(asm, "P_ADD") if line.endswith(" 4")])
        self.each(check)

    def test_a_call_through_a_function_pointer_is_far_where_code_is(self):
        def check(model, code_far, data_far):
            call = [line for line in body(self.compiled(model, "conventions.c")[1], "call_through") if line.startswith("call")]
            self.assertTrue(any("far" in line or "dword" in line for line in call) == code_far, call)
        self.each(check)


def body_of(asm, name):
    """Like `body` for a procedure spelled without the `_` (a pascal name)."""
    found = re.search(rf"^{name} proc.*?^{name} endp", asm, re.M | re.S)
    assert found, f"no procedure {name} in\n{asm}"
    return [line.strip() for line in found.group(0).splitlines()[1:-1] if line.strip() and not line.startswith("L")]


if __name__ == "__main__":
    unittest.main()
