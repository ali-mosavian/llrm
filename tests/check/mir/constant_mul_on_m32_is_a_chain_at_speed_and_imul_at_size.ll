; RUN: llrm-llc -O2 -march={i486 | pentium} --isel=dag -S -o /dev/stdout %s
; RUN: llrm-llc -Os -march=i486 --isel=dag -S -o /dev/stdout %s
; m32 has no exchange rate: at -O2 a constant multiply takes the fewest clocks (the lea/shl chain: x_iir was 2.17x gcc's clocks as `imul`),
; at -Os the pin's `imul` (bytes). Live control: constant_mul_is_a_chain_at_speed_on_m16.ll.
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
; CHECK[-O2]: {{lea|shl}}
; CHECK-NOT[-O2]: imul
; CHECK[-Os]: imul
; CHECK-LABEL: m251_ proc
; CHECK[-O2]: {{lea|shl}}
; CHECK-NOT[-O2]: imul
; CHECK[-Os]: imul
; CHECK-LABEL: m10000_ proc
; CHECK[-O2]: {{lea|shl}}
; CHECK-NOT[-O2]: imul
; CHECK[-Os]: imul
