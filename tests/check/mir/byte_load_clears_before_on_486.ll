; RUN: llrm-llc -O1 -march=i486 --isel=dag -S -o /dev/stdout %s
; RUN: llrm-llc -O1 -march=i486 --isel=old -S -o /dev/stdout %s
; RUN: llrm-llc -Os -march=i486 --isel=dag -S -o /dev/stdout %s
; RUN: llrm-llc -O1 -march=pentiumpro --isel=dag -S -o /dev/stdout %s
; DEVIATE ZL1 (peephole `zeroed_loads`, tune-gated by `clears_before_byte_load`) on both selectors: the pin keeps `movzx eax,
; byte ptr [..]`, which takes 3 clocks on the i486 (4 in 16-bit code) against 2 (3) for `xor eax, eax; mov al, [..]`.
; Kept for size and on the Pentium Pro (partial-register stall); the Pentium prices movzx at 3 as the 486 does and follows it. textfill c -O2 m16 checksum loop: +4000 clocks over main with movzx.
target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define i32 @f(ptr %p) {
  %b = load i8, ptr %p
  %z = zext i8 %b to i32
  ret i32 %z
}

; CHECK-LABEL: _f@
; CHECK[-O1 -march=i486]: xor eax, eax
; CHECK[-O1 -march=i486]: mov al, byte ptr
; CHECK[-Os]: movzx
; CHECK[pentiumpro]: movzx
