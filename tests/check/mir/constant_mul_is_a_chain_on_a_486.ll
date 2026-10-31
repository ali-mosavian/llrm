; RUN: llrm-llc -O2 -march={i486 | pentium} --isel=dag -S -o /dev/stdout %s
; RUN: llrm-llc -Os -march=i486 --isel=dag -S -o /dev/stdout %s
; At -Os bytes decide: the pin's cases cover them and the rest stays `imul` (qmove +19 B, bintree +12 with the chain).
; A multiply by 75, 251 or 10000 was `imul r,r,imm` (13-42 clocks on the 486, Intel 240440-002 Table 10.1); a lea/shl/add chain is 2-6.
; The sheet decides per CPU: the P5 prices 251 as shl/sub (lea pairs cost it more), the 486 as lea.
target datalayout = "e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32"
target triple = "i486-unknown-linux-gnu"

define i32 @m75(i32 %x) {
  %r = mul i32 %x, 75
  ret i32 %r
}
define i32 @m251(i32 %x) {
  %r = mul i32 %x, 251
  ret i32 %r
}
define i32 @m10000(i32 %x) {
  %r = mul i32 %x, 10000
  ret i32 %r
}

; CHECK-LABEL: m75_ proc
; CHECK-NOT[-O2]: imul
; CHECK[-O2]: lea
; CHECK[-Os]: imul
; CHECK-LABEL: m251_ proc
; CHECK-NOT[-O2]: imul
; CHECK[-O2 -march=i486]: lea
; CHECK[pentium]: shl
; CHECK[pentium]: sub
; CHECK[-Os]: imul
; CHECK-LABEL: m10000_ proc
; CHECK-NOT[-O2]: imul
; CHECK[-O2]: lea
; CHECK[-O2]: shl
; CHECK[-Os]: imul
