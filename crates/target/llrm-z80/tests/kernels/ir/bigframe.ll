source_filename = "bigframe.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"

; A 300-byte local: its end is out of the byte displacement of IX, so the spill slots below it and the
; element at 290 are reached through IY.
@buf = external global ptr

define void @touch(ptr %p) noinline optnone {
  store volatile ptr %p, ptr @buf
  ret void
}

define i8 @bigframe(i8 %v) {
  %buf = alloca [300 x i8]
  call void @touch(ptr %buf)
  %q = getelementptr [300 x i8], ptr %buf, i16 0, i16 10
  store i8 %v, ptr %q
  call void @touch(ptr %buf)
  %b = load i8, ptr %q
  ret i8 %b
}
