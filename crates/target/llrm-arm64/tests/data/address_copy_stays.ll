target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
@m = internal global [16 x i64] zeroinitializer, align 8
define void @f() {
  store ptr @m, ptr getelementptr inbounds (i8, ptr @m, i64 48), align 8
  %a = getelementptr inbounds i8, ptr @m, i64 8
  store i16 -1, ptr %a, align 2
  store ptr %a, ptr getelementptr inbounds (i8, ptr @m, i64 56), align 8
  %b = getelementptr inbounds i8, ptr @m, i64 16
  store i16 -1, ptr %b, align 2
  store ptr %b, ptr getelementptr inbounds (i8, ptr @m, i64 64), align 8
  ret void
}
