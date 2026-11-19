target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

@buf = external global [1024 x i8]

declare void @g(ptr)

; a store to buf + k + 500: two registers' worth of base (the PC-relative symbol and k) and a displacement that does
; not fit the 8 bits of an indexed mode
define void @f(i32 %k) {
  %i = add i32 %k, 500
  %p = getelementptr inbounds [1024 x i8], ptr @buf, i32 0, i32 %i
  store i8 9, ptr %p
  call void @g(ptr @buf)
  ret void
}
