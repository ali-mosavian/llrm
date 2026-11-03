source_filename = "halves.c"
target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"

@table = constant [3 x i16] [i16 1, i16 2, i16 3]
@cells = global [3 x i16] zeroinitializer

define i32 @get(i32 %i) {
  %p = getelementptr [3 x i16], ptr @table, i32 0, i32 %i
  %v = load i16, ptr %p
  %z = zext i16 %v to i32
  ret i32 %z
}

define void @put(i32 %i) {
  %p = getelementptr [3 x i16], ptr @cells, i32 0, i32 %i
  store i16 7, ptr %p
  ret void
}
