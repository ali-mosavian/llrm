"""Borland C that Open Watcom's front end refused, through wccq itself.

Skipped where owshim/build.sh has not built wccq or Borland's headers are
not installed.
"""

from pathlib import Path

import pytest

from qbopt.cfront import compile as cfront

BORLAND_H = Path.home() / "work" / "other" / "d32x" / "toolchains" / "bcpp31" / "INCLUDE"
pytestmark = pytest.mark.skipif(
    not cfront.WCCQ.exists() or not BORLAND_H.is_dir(), reason="needs owshim/bin/wccq and Borland's headers"
)


def _stream(tmp_path: Path, source: str) -> str:
    unit = tmp_path / "unit.c"
    unit.write_text(source)
    return cfront.recorded(unit, [str(BORLAND_H)])


def test_main_keeps_the_default_convention(tmp_path):
    """Open Watcom turns main into __watcall, `main_` with register parameters;
    Borland's c0 calls `_main` cdecl."""
    stream = _stream(tmp_path, "int main( void ) { return 0; }\n")
    symbol = next(line for line in stream.splitlines() if 'name="main"' in line)
    convention = stream.splitlines()[stream.splitlines().index(symbol) + 1]
    assert 'pattern="_*"' in symbol and "class=0x80" in convention and "parms=[]" in convention


def test_pointers_to_one_integer_size_convert(tmp_path):
    """input.c passes a `short *` for an `int *`: Borland warns, Open Watcom stopped with E1176."""
    source = "static short get( int *p ) { return *p; }\nshort use( short *k ) { return get( k ); }\n"
    assert "CGProcDecl" in _stream(tmp_path, source)


def test_inline_assembly_takes_387_instructions(tmp_path):
    """d_poly.c's `fsin` in __asm: E1156 invalid instruction with current CPU setting."""
    source = "double s( double r ) { double x; __asm { fld r\n fsin\n fstp x } return x; }\n"
    assert "CGProcDecl" in _stream(tmp_path, source)


def test_inline_assembly_frame_fixups_are_not_indirect_call_targets(tmp_path):
    """qcport's fsin inline body carries its input and output frame addresses
    on a call-like MIR node; they are fixups, not two indirect call targets."""
    text = _stream(
        tmp_path,
        "double s(double r) { double x; __asm { fld r\n fsin\n fstp x } return x; }\n",
    )

    assembly = cfront.compiled(text, "inline_fsin", optimise=True)

    assert "_s proc far" in assembly
    assert "call word ptr" not in assembly


def test_packed_aggregate_copy_promotes_the_word_leaves() -> None:
    """aggregatecopy kept ``[bp-8]`` and ``[bp-6]`` reloads after a dword copy."""
    source = Path("fixtures/c/aggregatecopy.c")
    assembly = cfront.compiled(cfront.recorded(source, []), source.stem, optimise=True)

    assert "word ptr [bp-8]" not in assembly
    assert "word ptr [bp-6]" not in assembly


def test_unbounded_far_aggregate_copy_remains_a_whole_access() -> None:
    """aggregatecopyfar must not turn one unbounded far dword read into two reads."""
    source = Path("fixtures/c/aggregatecopyfar.c")
    assembly = cfront.compiled(cfront.recorded(source, []), source.stem, optimise=True)

    assert "mov dword ptr [bp-8], eax" in assembly
    assert "mov eax, dword ptr es:[bx]" in assembly


def test_exact_pointer_aggregate_copy_promotes_the_word_leaves() -> None:
    """aggregatecopyptr kept local word reloads despite its one-object pointer proof."""
    source = Path("fixtures/c/aggregatecopyptr.c")
    assembly = cfront.compiled(cfront.recorded(source, []), source.stem, optimise=True)

    assert "word ptr [bp-14]" not in assembly
    assert "word ptr [bp-12]" not in assembly


def test_exact_pointer_destination_aggregate_copy_promotes_the_word_leaves() -> None:
    """aggregatecopydestptr kept local word reloads despite its one-object destination proof."""
    source = Path("fixtures/c/aggregatecopydestptr.c")
    assembly = cfront.compiled(cfront.recorded(source, []), source.stem, optimise=True)

    assert "word ptr [bp-8]" not in assembly
    assert "word ptr [bp-6]" not in assembly


def test_exact_pointer_aggregate_copy_keeps_both_address_dependencies() -> None:
    """aggregatecopybothptr needs one proven local pointer on each side of its copy."""
    source = Path("fixtures/c/aggregatecopybothptr.c")
    assembly = cfront.compiled(cfront.recorded(source, []), source.stem, optimise=True)

    assert "word ptr [bp-10]" not in assembly
    assert "word ptr [bp-8]" not in assembly
    assert "word ptr [bp-6]" not in assembly


def test_unreferenced_private_data_is_not_emitted() -> None:
    """private_data_dce retained static ``unusedValue`` after its sole
    externally visible function had been optimized.  An internal datum with
    no code or data relocation pointing at it has no OMF observer and must
    disappear, while the reachable static remains addressable.
    """
    source = Path("fixtures/c/private_data_dce.c")
    assembly = cfront.compiled(cfront.recorded(source, []), source.stem, optimise=True)

    assert "_unusedValue label byte" not in assembly
    assert "_retainedPointer label byte" in assembly
    assert "_retainedValue label byte" in assembly
    assert "word ptr _retainedPointer" in assembly
    unoptimised = cfront.compiled(cfront.recorded(source, []), source.stem)
    assert "_unusedValue label byte" in unoptimised


def test_constant_private_call_specializes_without_changing_dynamic_call() -> None:
    """ipconst_site left `zeroAdjusted` calling branchy `adjust` because a
    second dynamic call prevented whole-body parameter specialization.

    The constant call may instead clone into its caller and simplify there.
    That leaves one dynamic call, which the ordinary one-use inliner may then
    absorb without turning its runtime condition into a constant.
    """
    source = Path("fixtures/c/ipconst_site.c")
    assembly = cfront.compiled(cfront.recorded(source, []), source.stem, optimise=True)
    zero = assembly[assembly.index("_zeroAdjusted proc far") : assembly.index("_zeroAdjusted endp")]
    dynamic = assembly[assembly.index("_dynamicAdjusted proc far") : assembly.index("_dynamicAdjusted endp")]

    assert "call _adjust" not in zero
    assert "mov ax, 7" in zero
    assert "_adjust proc near" not in assembly
    assert "or ax, ax" in dynamic
    assert "add ax, 3" in dynamic


def test_returned_constant_specializes_the_next_private_call() -> None:
    """ipconst_chain kept a branchy private choose call after seed had already summarized to 4."""
    source = Path("fixtures/c/ipconst_chain.c")
    assembly = cfront.compiled(cfront.recorded(source, []), source.stem, optimise=True)
    chained = assembly[assembly.index("_chainedConstant proc far") : assembly.index("_chainedConstant endp")]

    assert "call _choose" not in chained
    assert "mov ax, 11" in chained
    assert "_choose proc near" not in assembly


def test_returned_constant_inlines_one_call_without_specializing_dynamic_callers() -> None:
    """ipconst_return_site kept a call to branchy choose after seed summarized to 4."""
    source = Path("fixtures/c/ipconst_return_site.c")
    assembly = cfront.compiled(cfront.recorded(source, []), source.stem, optimise=True)
    returned = assembly[
        assembly.index("_returnedConstantChoose proc far") : assembly.index("_returnedConstantChoose endp")
    ]
    dynamic = assembly[assembly.index("_dynamicChoose proc far") : assembly.index("_dynamicChoose endp")]

    assert "call _choose" not in returned
    assert "mov ax, 11" in returned
    assert "cmp ax, 4" in dynamic
    assert "add ax, 29" in dynamic


def test_unused_private_readonly_call_is_removed() -> None:
    """ipa_readonly kept sample's unobservable static load and call before returning 7."""
    source = Path("fixtures/c/ipa_readonly.c")
    assembly = cfront.compiled(cfront.recorded(source, []), source.stem, optimise=True)
    discard = assembly[assembly.index("_discardSample proc far") : assembly.index("_discardSample endp")]

    assert "call _sample" not in discard
    assert "mov ax, 7" in discard
    assert "_sample proc near" not in assembly

    volatile = assembly[assembly.index("_keepVolatileSample proc far") : assembly.index("_keepVolatileSample endp")]
    assert "call _sampleVolatile" in volatile


def test_private_nonreturn_call_prunes_its_unreachable_caller_tail() -> None:
    """ipa_noreturn left `mov ax, 7; retf` after a proven spinForever call."""
    source = Path("fixtures/c/ipa_noreturn.c")
    assembly = cfront.compiled(cfront.recorded(source, []), source.stem, optimise=True)
    caller = assembly[assembly.index("_entersSpin proc far") : assembly.index("_entersSpin endp")]

    assert "call _spinForever" in caller
    assert "mov ax, 7" not in caller
    assert "ret" not in caller


def test_private_terminal_recursion_prunes_its_callers_tail() -> None:
    """ipa_noreturn missed a→b→a and emitted `mov ax, 9; retf` in its caller."""
    source = Path("fixtures/c/ipa_noreturn_scc.c")
    assembly = cfront.compiled(cfront.recorded(source, []), source.stem, optimise=True)
    caller = assembly[assembly.index("_entersRecursiveSpin proc far") : assembly.index("_entersRecursiveSpin endp")]

    assert "call _spinFirst" in caller
    assert "mov ax, 9" not in caller
    assert "ret" not in caller


def test_relative_source_and_include(tmp_path, monkeypatch):
    """wccq runs in its scratch directory, where `fixtures/c/x.c` and `-I src`
    no longer resolved: E1051 unable to open."""
    (tmp_path / "src").mkdir()
    (tmp_path / "src" / "one.h").write_text("int one( void );\n")
    (tmp_path / "unit.c").write_text('#include "one.h"\nint two( void ) { return one() + 1; }\n')
    monkeypatch.chdir(tmp_path)
    assert "CGProcDecl" in cfront.recorded(Path("unit.c"), ["src"])


def test_borland_names_a_parameter_segment(tmp_path):
    """Borland's dos.h declares `peek( unsigned __segment, ... )`; `__segment`
    is an Open Watcom keyword, and mdl_ai.c stopped with E1060 Invalid type."""
    assert "CGProcDecl" in _stream(
        tmp_path, "int peek( unsigned __segment, unsigned __offset );\nint get( void ) { return peek( 0, 0x46c ); }\n"
    )


def test_stdc_is_undefined_as_in_bcc(tmp_path):
    """Open Watcom defines __STDC__ 1, which hid dos.h's FP_OFF macro: mdl_ai.c
    called an undefined `_FP_OFF` and QCPORT.EXE did not link."""
    stream = _stream(tmp_path, "#include <dos.h>\nunsigned off( void far *p ) { return FP_OFF( p ); }\n")
    assert "CGProcDecl" in stream and "FP_OFF" not in stream


def test_borland_conditional_keeps_explicit_far_pointer_stores(tmp_path):
    """qcport's CMD_FAR follows __BORLANDC__; without that target macro,
    cmd_init treated its far allocation as near and zeroed DGROUP instead."""
    text = _stream(
        tmp_path,
        "#ifdef __BORLANDC__\n"
        "#define TARGET_FAR far\n"
        "#else\n"
        "#define TARGET_FAR\n"
        "#endif\n"
        "void clear(unsigned char TARGET_FAR *p) { *p = 0; }\n",
    )

    assembly = cfront.compiled(text, "far_store", optimise=True)

    assert "TY_LONG_POINTER" in text
    assert "es:" in assembly


def test_compound_far_pointer_advance_preserves_its_segment(tmp_path):
    """qcport's ``srow += stride`` added ``DGROUP:stride`` to the packed
    pointer, changing its segment on every row; e1m1 wrote texture bytes into
    ROM after walking exposed a surface that took that lightmap-copy path.
    A far-pointer increment advances its offset and leaves its selector alone.
    """
    text = _stream(
        tmp_path,
        "void advance(unsigned char far **slot, unsigned stride) { *slot += stride; }\n",
    )

    assembly = cfront.compiled(text, "far_advance", optimise=True)
    body = assembly[assembly.index("_advance proc far") : assembly.index("_advance endp")]
    instructions = [line.strip() for line in body.splitlines()]

    assert "DGROUP" not in body, body
    assert any(
        line.startswith("add ") and not line.startswith(("add eax", "add ebx", "add ecx", "add edx"))
        for line in instructions
    ), body
    assert not any(line.startswith(("pop eax", "pop ebx", "pop ecx", "pop edx")) for line in instructions), body


def test_long_long_reaches_the_stream_as_signed_and_unsigned_int64(tmp_path):
    stream = _stream(
        tmp_path,
        "long long s(long long a) { return a + 1; }\nunsigned long long u(unsigned long long a) { return a * 3; }\n",
    )
    declarations = [line.split()[-1] for line in stream.splitlines() if "CGProcDecl" in line]
    parameters = [line.split()[-1] for line in stream.splitlines() if "CGParmDecl" in line]
    assert declarations == ["TY_INT_8", "TY_UINT_8"]
    assert parameters == ["TY_INT_8", "TY_UINT_8"]


def test_aggregate_copy_through_pointers_reaches_mir(tmp_path):
    """qcport's combat_brush_points stopped at `no scalar width for T51`
    while compiling `*target = *center`."""
    text = _stream(
        tmp_path,
        "typedef struct { float x, y, z; } Vec;\nvoid copy(Vec *a, Vec *b, Vec *src) { *a = *b = *src; }\n",
    )

    from qbopt.cfront import hir
    from qbopt.cfront import stream
    from qbopt.cfront import raise_hir

    unit = hir.unit(stream.parse(text))
    body = raise_hir.raised(unit, unit.procs[0]).body
    loads = [op for block in body.blocks for op in block.ops if op.kind is cfront.mir.Kind.LOAD]
    stores = [op for block in body.blocks for op in block.ops if op.kind is cfront.mir.Kind.STORE]
    assert sum(ref.width for op in loads for ref in op.loads if ref.width == 4) == 24
    assert sum(ref.width for op in stores for ref in op.stores) == 24


def test_aggregate_argument_is_pushed_by_value(tmp_path):
    """qcport's combat_radius passes a BspVec3 by value; the frontend stopped
    at `no scalar width for T51` instead of laying its 12 bytes on the stack."""
    text = _stream(
        tmp_path,
        "typedef struct { float x, y, z; } Vec;\nextern void take(Vec v);\nvoid pass(Vec *src) { take(*src); }\n",
    )

    from qbopt.cfront import hir
    from qbopt.cfront import stream
    from qbopt.cfront import raise_hir

    unit = hir.unit(stream.parse(text))
    body = raise_hir.raised(unit, unit.procs[0]).body
    args = [op for block in body.blocks for op in block.ops if op.kind is cfront.mir.Kind.ARG]
    assert [op.args[0].width for op in args] == [4, 4, 4]


def test_repeated_volatile_accesses_remain_observable(tmp_path):
    """``volatileValue + volatileValue`` was reduced to one memory read.

    A volatile lvalue is an observable access, not an ordinary load that GVN
    or scalar replacement may reuse, and one store may not replace another.
    Check optimized output rather than a frontend marker so the regression
    covers the complete C path.
    """
    text = _stream(
        tmp_path,
        "volatile unsigned volatileValue;\n"
        "unsigned readTwice(void) { return volatileValue + volatileValue; }\n"
        "void writeTwice(unsigned value) { volatileValue = value; volatileValue = value; }\n",
    )

    assembly = cfront.compiled(text, "volatile_reads", optimise=True)
    read_body = assembly[assembly.index("_readTwice proc far") : assembly.index("_readTwice endp")]
    write_body = assembly[assembly.index("_writeTwice proc far") : assembly.index("_writeTwice endp")]
    reads = [line for line in read_body.splitlines() if "_volatileValue" in line]
    writes = [line for line in write_body.splitlines() if "_volatileValue" in line]

    assert len(reads) == 2, read_body
    assert len(writes) == 2, write_body


def test_volatile_floating_accesses_retain_encodable_semantics(tmp_path):
    """The C floating benchmark stopped before lowering.

    Marking a volatile load/store by replacing its machine operation with a
    generic barrier preserved ordering but erased the ``fld``/``fstp`` shape
    that validates its IEEE conversion semantics.  Volatility and encoded
    computation are independent facts; optimized output must retain both.
    """
    text = _stream(
        tmp_path,
        "double once(void) { volatile double value = 1.0; value = value + 0.5; return value; }\n",
    )

    assembly = cfront.compiled(text, "volatile_float", optimise=True)
    body = assembly[assembly.index("_once proc far") : assembly.index("_once endp")]

    assert body.count("fld ") >= 2, body
    assert "fadd" in body, body
    assert "fstp" in body, body


def test_near_function_pointer_call_reaches_the_emitter(tmp_path):
    """qcport's pl_items_touch calls ItemInfo.take through a near pointer;
    the frontend stopped at `indirect call` instead of emitting `call r/m16`."""
    text = _stream(
        tmp_path,
        "typedef int (near *Take)(int);\nint apply(Take take, int value) { return take(value); }\n",
    )

    assembly = cfront.compiled(text, "indirect", optimise=True)
    assert any(line.strip().startswith("call ") and "_take" not in line for line in assembly.splitlines())


def test_far_function_designator_cast_to_a_long_reaches_the_emitter(tmp_path):
    """qcport snd.c passes its far DSP callback as a packed long address.

    A function designator is not yet a scalar value.  The raiser must first
    materialize its relocatable far address, then let the ordinary long
    argument path push the segment:offset pair.
    """
    text = _stream(
        tmp_path,
        "typedef void (far *Callback)(void);\n"
        "static void far cdecl cb(void) {}\n"
        "extern void callback(long);\n"
        "void use(void) { callback((long)(Callback) cb); }\n",
    )

    assembly = cfront.compiled(text, "far_callback", optimise=True)

    body = assembly[assembly.index("_use proc far") : assembly.index("_use endp")]
    assert "pushw seg _cb" in body and "push offset _cb" in body


def test_far_function_designator_decays_and_calls_indirectly(tmp_path):
    """qcport host.c passes a far callback without an explicit cast, while
    item.c later calls it through the parameter.  The frontend used to reject
    the first Function as a scalar value and the second as a non-near call.
    Both are ordinary C function-pointer decay under the medium-model ABI.
    """
    text = _stream(
        tmp_path,
        "typedef void (far *Notice)(int);\n"
        "static void far notify(int value) { (void)value; }\n"
        "void take(Notice notice) { notice(7); }\n"
        "void pass(void) { take(notify); }\n",
    )

    assembly = cfront.compiled(text, "far_indirect", optimise=True)
    take = assembly[assembly.index("_take proc far") : assembly.index("_take endp")]
    passed = assembly[assembly.index("_pass proc far") : assembly.index("_pass endp")]

    assert "call dword ptr [bp" in take
    assert "pushw seg _notify" in passed and "push offset _notify" in passed


def test_discarded_float_call_result_is_popped(tmp_path):
    """qcport gib.c discards gib_crandom's float result.  Float allocation
    correctly produced ``fstp st(0)``, but selection had no encoding for the
    x87 register form and the optimized build stopped before writing an OBJ.
    """
    text = _stream(
        tmp_path,
        "extern float sample(void);\nvoid discard(void) { (void)sample(); }\n",
    )

    assembly = cfront.compiled(text, "discard_float", optimise=True)
    body = assembly[assembly.index("_discard proc far") : assembly.index("_discard endp")]

    assert "fstp st(0)" in body


def test_external_far_object_uses_its_own_selector(tmp_path):
    """qcport linked mon_facts against DGROUP even though its declaration is
    far; the linker rejected the offset's group frame as a different segment."""
    text = _stream(tmp_path, "extern int far y;\nvoid far *address(void) { return &y; }\n")

    from qbopt.backend import masm
    from qbopt.objectfile import omf
    from qbopt.backend import omfwrite

    built = cfront.assembled(text, "far_object", optimise=True)
    assembly = masm.text(built)
    records = omf.parse(omfwrite.written(built, "far_object.c"))
    externals = omf.externals(records)
    y = externals.index("_y")
    relocations = [one for one in omf.fixups(records) if one.target == "external" and one.index == y]

    assert "mov dx, seg _y" in assembly
    assert "mov dx, DGROUP" not in assembly
    assert relocations and all(one.frame_method != 1 for one in relocations)


def test_borland_intrinsic_runtime_name_is_normalized(tmp_path):
    """dos.h spells outportb as compiler intrinsic __outportb__, while the
    Borland medium-model runtime exports the callable fallback as _outportb."""
    text = _stream(tmp_path, "#include <dos.h>\nvoid send(void) { outportb(0x3f8, 'x'); }\n")

    assembly = cfront.compiled(text, "outport", optimise=True)

    assert "call far ptr _outportb" in assembly
    assert "___outportb__" not in assembly


def test_restrict_reaches_mir_as_distinct_noalias_roots(tmp_path):
    """OW parsed restrict but discarded it before CG; the shim now records it."""
    source = "void add(int *__restrict out, const int *__restrict a, const int *__restrict b) {\n  *out = *a + *b;\n}\n"
    text = _stream(tmp_path, source)
    assert text.count(" CGAttr ") == 3

    from qbopt.cfront import hir
    from qbopt.cfront import stream
    from qbopt.cfront import raise_hir

    unit = hir.unit(stream.parse(text))
    body = raise_hir.raised(unit, unit.procs[0]).body
    roots = {
        next(iter(ref.provenance.restrict))
        for block in body.blocks
        for op in block.ops
        for ref in (*op.loads, *op.stores)
        if ref.provenance is not None and ref.provenance.restrict
    }
    assert len(roots) == 3


def test_standard_allocator_return_has_fresh_object_identity(tmp_path):
    """A pointer returned by malloc is an allocation-site object, not an unknown pointer."""
    text = _stream(
        tmp_path,
        "extern void *malloc(unsigned n);\nint *make(void) { return (int *)malloc(8); }\n",
    )

    from qbopt.cfront import hir
    from qbopt.model import memory
    from qbopt.cfront import stream
    from qbopt.cfront import raise_hir

    unit = hir.unit(stream.parse(text))
    body = raise_hir.raised(unit, unit.procs[0]).body
    allocations = {
        slice_.object
        for provenance in body.pointer_seeds.values()
        for slice_ in provenance.slices
        if slice_.object.kind is memory.Kind.ALLOCATION
    }

    assert len(allocations) == 1
    assert next(iter(allocations)).extent == 8
