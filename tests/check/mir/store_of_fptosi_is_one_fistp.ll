; RUN: llrm-llc -O1 -march=i486 --isel=dag -S -o /dev/stdout %s
; A float `fptosi` stored to a global was `fistp` to a frame temp, a reload, then the store; now one `fistp` into the global.
target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@g16 = global i16 0
@g32 = global i32 0

define void @s16(float %x) {
  %i = fptosi float %x to i16
  store i16 %i, ptr @g16
  ret void
}

define void @s32(float %x) {
  %i = fptosi float %x to i32
  store i32 %i, ptr @g32
  ret void
}

; The control-word saves stay in the frame; the converted value does not.
; CHECK-LABEL: _s16@
; CHECK: fistp word ptr _g16
; CHECK-NOT: mov word ptr _g16
; CHECK-LABEL: _s32@
; CHECK: fistp dword ptr _g32
; CHECK-NOT: mov dword ptr _g32
