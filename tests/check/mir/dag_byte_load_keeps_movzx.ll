; RUN: llrm-llc -O1 -march=i486 --isel=dag -S -o /dev/stdout %s
; RUN: llrm-llc -O1 -march=i486 --isel=old -S -o /dev/stdout %s
; The DAG selector (the default) loaded an unsigned byte as `xor eax, eax; mov al, [..]`, where the pin keeps `movzx eax, byte ptr [..]`:
; DEVIATE ZL1 (peephole `zeroed_loads`) belongs to the old selector. The second RUN keeps that deviation.
target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define i32 @f(ptr %p) {
  %b = load i8, ptr %p
  %z = zext i8 %b to i32
  ret i32 %z
}

; CHECK-LABEL: _f@
; CHECK[--isel=old]: xor eax, eax
; CHECK[--isel=old]: mov al, byte ptr
; CHECK[--isel=dag]: movzx
