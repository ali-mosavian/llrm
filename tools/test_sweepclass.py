import sweepclass


def test_a_dos_only_source_and_a_missing_header_are_na_and_a_backend_refusal_is_fail():
    """A wasm sweep counted 168 FAIL cells, 158 of them sources no backend can compile (far/regparm/__emit__, stdio.h off the include path): the column hid the 10 real gaps.
    Only the front end's rejection is NA; a Legalize error or an fp128 the MIR subset refuses stays FAIL."""
    assert sweepclass.classify("tests/run/c/watcom_far_sum.c:4:18: error: expected ')'\n") == "NA"
    assert sweepclass.classify("tests/run/c/x.c:2:10: fatal error: 'stdio.h' file not found\n") == "NA"
    assert sweepclass.classify("tests/run/c/inline_emit.c:9:5: error: call to undeclared function '__emit__'\n") == "NA"
    assert sweepclass.classify('llrm-wasm-llc: @f: Legalize("Unsupported { op: BSwap }")\n') == "FAIL"
    assert sweepclass.classify("llrm-wasm-llc: line 25: `fp128` is outside MIR's subset of LLVM\n") == "FAIL"
    assert sweepclass.classify("") == "FAIL"


def test_a_backend_refusal_after_a_front_end_warning_line_is_still_fail():
    """The first diagnostic decides: a note about the source must not turn a backend failure into NA."""
    assert sweepclass.classify("x.c:1:1: note: expected here\nllrm-wasm-llc: @f: Legalize(\"x\")\n") == "FAIL"
