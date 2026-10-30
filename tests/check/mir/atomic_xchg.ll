; RUN: llrm-llc -O2 -S -o /dev/stdout %s
; An `atomicrmw xchg` and a `seq_cst` store were refused ("is not lowered for this target yet"). Each is an `xchg` with
; the cell, which the machine locks; the fence short of `seq_cst` is nothing.
target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@flag = global i16 0

define i16 @take(i16 %v) {
entry:
  %old = atomicrmw xchg ptr @flag, i16 %v acquire
  fence acq_rel
  store atomic i16 %v, ptr @flag seq_cst, align 2
  ret i16 %old
}

; CHECK-LABEL: _take@
; CHECK: xchg {{.*}}, word ptr _flag
; CHECK: xchg {{.*}}, word ptr _flag
