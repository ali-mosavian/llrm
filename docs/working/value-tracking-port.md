# ValueTracking port: evidence

The evidence behind the ValueTracking, KnownBits and ConstantRange translation from LLVM `d1106deb71cc`: agreement with LLVM, deviations and their exits, owed work.

**General rule:** a fact LLVM proves of a value at a context, llrm proves, by the same cases in the same order; one translation serves InstSimplify's icmp gaps, CE, demanded bits, SCCP/LVI/CVP/SCEV ranges. Covers every integer or pointer value of any MIR program, not the cases a consumer asks about today.

- `knownbits.rs`, `constantrange.rs` (+ `apint.rs`): every operation hashed against LLVM over every input of widths 1-5 / 1-4 and samples to 128 bits (900 + 1,488 hashes). `multiply`, `mulhs`, `mulhu`, the averages run at twice the width in a 256-bit word, as LLVM does.
- `valuetracking/`: `known` (computeKnownBits), `signbits`, `nonzero` + `power2`, `range` (computeConstantRange, matchSelectPattern, overflow), `context` (assume, dominating conditions, isImpliedCondition, AssumptionCache), `guarantee` (isGuaranteedNotTo*, execution transfer), `pattern` (`m_*`), `query` (SimplifyQuery). Depth cap 6.
- Oracle: `tools/llvmport/` C++ drivers linked to the pinned LLVM (24.0.0git `d1106deb71cc`, never the system LLVM) beside the Rust `vtdump` example; `vtcompare.py` diffs the lines. Tests assert LLVM's lines on inputs in `tests/vt/`.
- The `facts_rewrite` flag-reader audit is done for the 14 reads value tracking makes (module doc); the guard test passes.

**Agreement with LLVM** on LLVM's own tests (values compared / differing; every difference is llrm claiming less):

| col | Analysis/ValueTracking | InstSimplify | InstCombine | CVP | SCCP |
|---|---|---|---|---|---|
| kb | 2052/4 | 5738/3 | 37142/58 | 941/0 | 1018/2 |
| sb | 2052/0 | 5738/0 | 37142/20 | 941/0 | 1018/0 |
| nz | 2052/1 | 5738/0 | 37142/10 | 941/0 | 1018/2 |
| p2 / p2z | 2052/0 / 0 | 5738/0 / 0 | 37142/0 / 0 | 941/0 / 0 | 1018/0 / 1 |
| nn / ng | 2052/3 / 0 | 5738/1 / 0 | 37142/19 / 2 | 941/0 / 0 | 1018/1 / 0 |
| cru, crs | 2021/1 | 5531/0 | 34402/1 | 852/0 | 944/1 |
| gp | 2052/31 | 5738/4 | 37142/229 | 941/4 | 1018/1 |

Causes of the differences, by size: (1) `llvm.*` declarations MIR has not given LLVM's attributes (#2142); (2) LLVM's parse-time upgrades (`x86.bmi.pext/pdep`); (3) (closed: `DataLayout::pref_align`); (4) float-to-int bitcast needs `computeKnownFPClass`.

**Not covered:** 1,137 of 2,473 test files do not parse in MIR: `undef` 169, `target triple` 126 (strippable), vectors (`shufflevector`/`insertelement`/`extractelement`) ~124, `vscale` 67, integers wider than 128 bits 40, attributes MIR lacks (`null_pointer_is_valid`, `strictfp`, `nofpclass`, `dso_local`, ...) ~100, `half`/`fpN` 33, functions declared nowhere 112. Vector and `undef` code paths are untested against LLVM here.

**DEVIATE** follow oversight's rule (only where MIR or the target lacks the construct; each states its MIR fact and has a test in `valuetracking/boundary_tests.rs` or `mir_holds_no_integer_wider_than_128` that fails when MIR gains it):
1. Widths above 128: MIR parses `i1` to `i128`. Exit: #2200.
2. Lane operations (`extractelement`, `insertelement`, `shufflevector`), scalable vectors, `vscale_range`: MIR has none. MIR types vectors and verifies element-wise operations, but no front end or lowering produces or selects one, so `DemandedElts` is not translated and a vector value claims nothing. `nothing_outside_mir_produces_a_vector_type` fails the day one is first built; `DemandedElts` is then the next translation. Exit: #2201.
3. `undef`: MIR has `poison` and `freeze`. Exit: #2202.
4. Operand bundles. Exit: #2203.
5. `ptrtoaddr`. Exit: #2204.
6. `read_register`: MIR keeps no target triple. Exit: #2205.
7. `half`. Exit: #2207.
8. Preferred alignment: `DataLayout` has none (a global gets its ABI alignment; about 25 InstCombine pointer cases claim less). Exit: #2208.

**OWED** (#2209; MIR has the construct; to be translated): `computeKnownFPClass` and what it gates: `isKnownNeverNaN/Inf`, `isImpliedCondFCmps` (needs `ConstantFPRange`), the FP uses of `Q.CC`, float-to-int bitcast and `fptosi` known bits (5 lines in `minmax-fp.ll`). MIR has `float` and `double`, so this is precision lost, not an absent construct.

`SimplifyQuery::CC` is translated here (`CondContext`, `without_cond_context` at the 7 sites, checked against LLVM by `ctxdump cc`: 74 lines in `condctx.ll`; corpus 0 differing on ValueTracking/CVP/SCCP).

`FunctionAnalyses` (dominator tree + assumptions + layout, built once per function) is the entry point a pass uses: `analyses.query(&module, function, Some(inst))`.

**Fail-first tests** (each asserts LLVM's own answers, which the conservative stubs they replaced did not give): `knownbits_tests::test_every_operation_agrees_with_llvm` (900 hashes), `constantrange_tests::test_every_operation_agrees_with_llvm` (1,488), `known_tests`, `signbits_tests`, `nonzero_tests`, `power2_tests`, `range_tests`, `guarantee_tests`, `context_tests` (assume, order, `condctx`: 74 lines of `CondContext`, `FunctionAnalyses`), `boundary_tests`. Premise rule: `the_pinned_llvm_answers_we_assert_say_more_than_one_thing` and the distinct-hash asserts fail if an oracle file could only say one thing (every column has at least two answers; the hash files have more than half distinct). The comparison is against the pinned LLVM only (24.0.0git `d1106deb71cc`); each driver answers `--version`.

No consumer changes: nothing outside `llrm-mir` calls the new functions; the old `known_zero`/`sign_bits` stay until their callers switch (one PR each, starting with InstSimplify's icmp gaps).

