"""The clang table counts a pass by the IR it ran over (the dump before it), and takes a phase from the callgrind report."""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import clang_pass_budgets as cpb  # noqa: E402

DUMP = """\
; *** IR Dump Before InstCombinePass on f ***
define i32 @f(i32 %x) {
entry:
  %a = add i32 %x, 1
  ret i32 %a
}
; *** IR Dump Before LICMPass on loop ***
; Preheader:
body:
  %b = mul i32 %x, 2
  br label %body
; *** IR Dump Before InstCombinePass on g ***
define i32 @g() {
  ret i32 0
}
"""

REPORT = """\
555,000,000 (100.0%)  PROGRAM TOTALS
230,000,000 (41.4%)  ???:llvm::PassManager<llvm::Module, llvm::AnalysisManager<llvm::Module>>::run(llvm::Module&, llvm::AnalysisManager<llvm::Module>&) [/usr/lib/llvm-20/bin/clang]
 47,000,000 ( 8.5%)  ???:llvm::InstCombinePass::run(llvm::Function&, llvm::AnalysisManager<llvm::Function>&) [/usr/lib/llvm-20/bin/clang]
244,000,000 (43.9%)  ???:llvm::legacy::PassManager::run(llvm::Module&) [/usr/lib/llvm-20/bin/clang]
"""


def test_a_pass_is_counted_by_its_dumps_and_the_instructions_in_them():
    """Comments and labels are no instructions: InstCombine ran twice over 3 of them, LICM once over 2."""
    assert cpb.dumped(DUMP) == {"InstCombinePass": (2, 3), "LICMPass": (1, 2)}


def test_the_phases_are_the_optimizer_codegen_and_the_front_end_that_is_left():
    """`total` was read as 0 (its row has no `file:` part), which made the front end -474 M."""
    passes, phases, legacy = cpb.costs(REPORT)
    assert passes == {"InstCombinePass": 47.0} and legacy == {}
    assert (phases["optimizer"], phases["codegen"], phases["total"], phases["front end"]) == (230.0, 244.0, 555.0, 81.0)
