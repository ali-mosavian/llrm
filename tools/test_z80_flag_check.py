import z80_flag_check as z

# The command line of a z80 corpus cell: CLANG_FLAGS (run.py) plus the program's -f flags; and the four SDCC variants (z80-abf-lci.py).
OURS = ["-O2", "-target", "msp430", "-fsigned-char", "-fpack-struct=1", "-fno-vectorize", "-fno-slp-vectorize", "-Xclang", "-disable-llvm-passes",
        "-Xclang", "-disable-O0-optnone", "-fomit-frame-pointer", "-fno-stack-protector", "-fno-asynchronous-unwind-tables", "-S", "-emit-llvm"]
VARIANTS = [["--fsigned-char", "--opt-code-size"], ["--fsigned-char", "--opt-code-size", "--max-allocs-per-node", "200000"],
            ["--fsigned-char", "--opt-code-speed"], ["--fsigned-char", "--opt-code-speed", "--max-allocs-per-node", "200000"]]


def test_the_real_cell_flags_agree():
    assert all(z.sdcc_mismatch(OURS, v) == "" for v in VARIANTS)
    assert z.sdcc_mismatch(OURS + ["-fno-inline-functions", "-fwrapv"], VARIANTS[0]) == ""


def test_an_sdcc_only_flag_is_refused_not_measured():
    assert "--stack-auto" in z.sdcc_mismatch(OURS, ["--fsigned-char", "--opt-code-size", "--stack-auto"])


def test_a_char_signedness_on_one_side_only_is_refused():
    assert "signedness" in z.sdcc_mismatch(OURS, ["--opt-code-size"])
    assert "signedness" in z.sdcc_mismatch([f for f in OURS if f != "-fsigned-char"], VARIANTS[0])


def test_an_ours_only_flag_without_a_listed_counterpart_is_refused():
    """-funroll-loops would change ours and not SDCC; -ffixed-point has no SDCC type at all."""
    assert "-funroll-loops" in z.sdcc_mismatch(OURS + ["-funroll-loops"], VARIANTS[0])
    assert "-ffixed-point" in z.sdcc_mismatch(OURS + ["-ffixed-point"], VARIANTS[0])


def test_a_variant_needs_exactly_one_goal():
    assert z.sdcc_mismatch(OURS, ["--fsigned-char"])
    assert z.sdcc_mismatch(OURS, ["--fsigned-char", "--opt-code-size", "--opt-code-speed"])


GCC = ["-O2", "-fsigned-char", "-fpack-struct=1", "-fno-tree-loop-distribute-patterns", "-w", "-std=gnu17"]


def test_gcc_gets_the_programs_f_flags_too():
    ours = OURS + ["-fno-inline-functions", "-fwrapv"]
    assert z.gcc_mismatch(ours, GCC + ["-fno-inline-functions", "-fwrapv"]) == ""
    assert "program flags" in z.gcc_mismatch(ours, GCC)
    assert "level" in z.gcc_mismatch(OURS, ["-Os"] + GCC[1:])

    assert "program flags" in z.gcc_mismatch(OURS, ["-O2", "-fno-ipa-cp", "-fsigned-char", "-fpack-struct=1"])
