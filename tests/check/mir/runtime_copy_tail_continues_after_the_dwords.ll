; RUN: llrm-llc -O2 -march=i486 --isel=dag -S -o /dev/stdout %s
; A runtime-length memcpy inlined as `rep movsd` and an n&3 `rep movsb` reloaded di/si with the copy's start for the tail:
; the tail rewrote the first bytes and the last n&3 were never copied (n=17, d[i+1]=s[i+3]: d[17] stayed old).
; The tail now starts at dst+(n-n%4), src+(n-n%4).
target datalayout = "e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32"
target triple = "i486-unknown-linux-gnu"

declare void @llvm.memcpy.p0.p0.i32(ptr noalias nocapture writeonly, ptr noalias nocapture readonly, i32, i1 immarg)

define void @copy(ptr %d, ptr %s, i32 %n) {
  call void @llvm.memcpy.p0.p0.i32(ptr align 1 %d, ptr align 1 %s, i32 %n, i1 false)
  ret void
}

; CHECK-LABEL: copy_ proc
; CHECK: mov edi, eax
; CHECK: mov esi, edx
; CHECK: rep movs dword
; CHECK-NOT: mov edi, eax
; CHECK-NOT: mov esi, edx
; CHECK: rep movs byte
