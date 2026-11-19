target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
declare void @llvm.va_start.p0(ptr)
declare void @llvm.va_end.p0(ptr)

; the named word is in hl; the unnamed ones follow on the stack
define i16 @first(i16 %n, ...) {
  %ap = alloca ptr, align 1
  call void @llvm.va_start.p0(ptr %ap)
  %v = va_arg ptr %ap, i16
  call void @llvm.va_end.p0(ptr %ap)
  %r = add i16 %v, %n
  ret i16 %r
}

; the second word would fit in de, but it is unnamed: both go on the stack
define i16 @caller(i16 %a, i16 %b, i16 %c) {
  %r = call i16 (i16, ...) @first(i16 %a, i16 %b, i16 %c)
  ret i16 %r
}
