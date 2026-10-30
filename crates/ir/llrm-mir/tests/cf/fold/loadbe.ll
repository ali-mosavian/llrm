target datalayout = "E-p:64:64-i64:64"
@g = constant [4 x i32] [i32 1, i32 2, i32 3, i32 4]
@s = constant { i16, i8, i32 } { i16 258, i8 3, i32 67305985 }
define void @t() {
  %a = load i32, ptr @g
  %b = load i16, ptr getelementptr inbounds (i8, ptr @g, i64 1)
  %c = load i64, ptr @g
  %d = load i32, ptr @s
  %e = load i32, ptr getelementptr inbounds (i8, ptr @s, i64 2)
  %f = load i8, ptr getelementptr inbounds (i8, ptr @s, i64 7)
  ret void
}
