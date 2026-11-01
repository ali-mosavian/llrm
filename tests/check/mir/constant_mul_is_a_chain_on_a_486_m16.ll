; RUN: llrm-llc -O2 -march=i486 --isel=dag -S -o /dev/stdout %s
; RUN: llrm-llc -Os -march=i486 --isel=dag -S -o /dev/stdout %s
; m16: a multiply by 75, 251 or 10000 is a chain at -O2 (an `imul r16` is 13-26 clocks on the 486); -Os keeps `imul` (the chains grow the object).
target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define i16 @m75(i16 %x) {
  %r = mul i16 %x, 75
  ret i16 %r
}
define i16 @m251(i16 %x) {
  %r = mul i16 %x, 251
  ret i16 %r
}
define i16 @m10000(i16 %x) {
  %r = mul i16 %x, 10000
  ret i16 %r
}

; CHECK-LABEL: _m75@
; CHECK-NOT[-O2]: imul
; CHECK[-O2]: sh
; CHECK[-Os]: imul
; CHECK-LABEL: _m251@
; CHECK-NOT[-O2]: imul
; CHECK[-O2]: sh
; CHECK[-Os]: imul
; CHECK-LABEL: _m10000@
; CHECK-NOT[-O2]: imul
; CHECK[-O2]: sh
; CHECK[-Os]: imul
