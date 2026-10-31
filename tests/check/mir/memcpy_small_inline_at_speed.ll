; RUN: llrm-llc -O1 -march=i486 --isel=dag -S -o /dev/stdout %s
; RUN: llrm-llc -Os -march=i486 --isel=dag -S -o /dev/stdout %s
; A constant copy of 14 bytes is dword and word moves at speed (4 pieces, the pin's limit of 8) and `rep movs` for size
; (m16's limit of 2 pieces, SD223/SD225): it was `rep movs byte` at every level, 14 bytes at 3 clocks + 12 of setup on the 486.
; lru nib -O2 m16: 447 -> 356 clocks, 144 -> 104 memory operands.
target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

declare void @llvm.memcpy.p1.p1.i16(ptr addrspace(1) noalias nocapture writeonly, ptr addrspace(1) noalias nocapture readonly, i16, i1 immarg)
declare void @llvm.memcpy.p0.p0.i16(ptr noalias nocapture writeonly, ptr noalias nocapture readonly, i16, i1 immarg)

define void @far(ptr addrspace(1) %d, ptr addrspace(1) %s) {
  call void @llvm.memcpy.p1.p1.i16(ptr addrspace(1) align 2 %d, ptr addrspace(1) align 2 %s, i16 14, i1 false)
  ret void
}

define void @near(ptr %d, ptr %s) {
  call void @llvm.memcpy.p0.p0.i16(ptr align 2 %d, ptr align 2 %s, i16 14, i1 false)
  ret void
}

; CHECK-LABEL: _far@
; CHECK-NOT[-O1]: rep
; CHECK[-Os]: rep movs
; CHECK-LABEL: _near@
; CHECK-NOT[-O1]: rep
; CHECK[-Os]: rep movs
