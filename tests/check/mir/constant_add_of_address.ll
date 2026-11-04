; RUN: llrm-llc -O2 -S -o /dev/stdout %s
; A constant expression `add (ptrtoint @a), 3` (clang prints one for `(int)a + 3`) is an add of its operands, not a refusal: llrm-llc stopped
; on "expected a constant, found `add`" (torture loop-2d).
target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@a = global [2 x i16] zeroinitializer
@r = global i16 0

; CHECK-LABEL: _f@3 proc
; CHECK: mov word ptr _r, offset _a+3
define void @f() {
  store i16 add (i16 ptrtoint (ptr @a to i16), i16 3), ptr @r
  ret void
}
