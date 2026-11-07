target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"

@xs = constant [4 x i32] [i32 1, i32 2, i32 3, i32 4]

define i32 @f(i16 %i) {
  %p = getelementptr inbounds [4 x i8], ptr @xs, i16 %i
  %v = load i32, ptr %p
  ret i32 %v
}
