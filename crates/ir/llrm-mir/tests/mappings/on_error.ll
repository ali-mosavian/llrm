; ON ERROR: a call that may raise is an invoke; its handler is a
; landingpad under llrm's personality. The interpreter cannot unwind, so
; this runs the normal path only, and msp430 has no exception handling.
; expect: 4
; msp430: no
target datalayout = "e-p:16:16-p1:32:16:16:16-i32:16-i64:16"

declare i32 @llrm.qb.personality(...)
declare void @exit(i32) noreturn

define i16 @llrm.qb.divide(i16 %a, i16 %b) {
  %q = sdiv i16 %a, %b
  ret i16 %q
}

define i32 @main() personality ptr @llrm.qb.personality {
entry:
  %q = invoke i16 @llrm.qb.divide(i16 8, i16 2) to label %ok unwind label %handler
ok:
  %w = zext i16 %q to i32
  ret i32 %w
handler:
  %error = landingpad { ptr, i32 } catch ptr null
  %code = extractvalue { ptr, i32 } %error, 1
  ret i32 %code
}
