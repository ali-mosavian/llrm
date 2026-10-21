; RUN: opt < %s -disable-output "-passes=print<da>" 2>&1 | FileCheck %s

; Test case for wrapping AddRec detection in DependenceAnalysis.
; This ensures that AddRec expressions that wrap (creating cyclic rather than
; linear patterns) are rejected from SIV analysis and treated conservatively.

; This test case has a clear dependence pattern that was incorrectly reported as "none!"
; The issue: {false,+,true} in i1 arithmetic creates pattern (0,1,0,1,0,1,...).
; - i=0: a[0][0][0], i=1: a[0][1][1], i=2: a[0][0][0], i=3: a[0][1][1], ...
; - Clear dependencies at distances 2, 4, 6 between iterations accessing same locations.
; - Strong SIV test was missing these due to treating wrapping pattern as linear.

define void @test_wrapping_i1_addrec(ptr %a) {
;
entry:
  br label %loop

loop:
  %i = phi i64 [ 0, %entry ], [ %i.next, %loop ]
  %and = and i64 %i, 1
  %idx = getelementptr inbounds [4 x [4 x i8]], ptr %a, i64 0, i64 %and, i64 %and
  store i8 0, ptr %idx
  %i.next = add i64 %i, 1
  %exitcond.not = icmp slt i64 %i.next, 8
  br i1 %exitcond.not, label %loop, label %exit

exit:
  ret void
}