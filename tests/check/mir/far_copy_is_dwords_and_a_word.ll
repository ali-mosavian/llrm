; RUN: llrm-llc -O1 -march=i486 --isel=dag -S -o /dev/stdout %s
; RUN: llrm-llc -Os -march=i486 --isel=dag -S -o /dev/stdout %s
; A constant 14-byte copy between far pointers (align 2, beyond the pieces limit) was `rep movsb` of 14: 3 clocks a byte on the 486.
; The near copy was already `rep movsd` and a tail; the far one is now `rep movsd` of 3 and a `movsw` glued after it (the
; segment registers are set once), as at -Os. lru nib -O2 m16: 447 -> 417 clocks, 144 -> 104 memory operands, 596 -> 602 B.
target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

declare void @llvm.memcpy.p1.p1.i16(ptr addrspace(1) noalias nocapture writeonly, ptr addrspace(1) noalias nocapture readonly, i16, i1 immarg)

define void @far(ptr addrspace(1) %d, ptr addrspace(1) %s) {
  call void @llvm.memcpy.p1.p1.i16(ptr addrspace(1) align 2 %d, ptr addrspace(1) align 2 %s, i16 14, i1 false)
  ret void
}

; CHECK-LABEL: _far@
; CHECK: rep movs dword
; CHECK: movs word
; CHECK-NOT: byte ptr
