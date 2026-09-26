; A calling convention is a number on the callee and the call, as LLVM's
; are: cc1000 is BASIC's, pushed left to right and popped by the callee.
; A far function lives in address space 1, whose pointers are
; segment:offset. msp430 knows neither.
; expect: 7
; msp430: no
target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

define internal cc1000 i16 @difference(i16 %a, i16 %b) addrspace(1) {
  %d = sub i16 %a, %b
  ret i16 %d
}

define internal fastcc i16 @twice(i16 inreg %a) {
  %d = add i16 %a, %a
  ret i16 %d
}

define i16 @main() {
  %x = call cc1000 addrspace(1) i16 @difference(i16 10, i16 3)
  %y = call fastcc i16 @twice(i16 inreg %x)
  %z = sub i16 %y, %x
  ret i16 %z
}
