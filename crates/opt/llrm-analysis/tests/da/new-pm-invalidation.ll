; RUN: opt < %s -passes='require<da>,invalidate<scalar-evolution>,print<da>'   \
; RUN:   -disable-output -debug-pass-manager 2>&1 | FileCheck %s

; This test cannot be converted to use utils/update_analyze_test_checks.py
; because the pass order printing is not deterministic.

define void @test_no_noalias(ptr %A, ptr %B) {
  store i32 1, ptr %A
  store i32 2, ptr %B
  ret void
}