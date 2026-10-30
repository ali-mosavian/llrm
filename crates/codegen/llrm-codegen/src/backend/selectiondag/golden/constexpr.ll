target datalayout = "e-m:e-p:32:32-f64:32:64-f80:32-n8:16:32-S128"
target triple = "i486-unknown-linux-gnu"

@g = global [32 x i8] zeroinitializer
@arr = global [16 x i32] zeroinitializer
@p = global ptr null

define void @store_gep() {
  store i32 5, ptr getelementptr inbounds nuw (i8, ptr @g, i32 8)
  store ptr getelementptr inbounds nuw (i8, ptr @g, i32 4), ptr @p
  ret void
}

define i16 @load_gep() {
  %v = load i16, ptr getelementptr inbounds nuw (i8, ptr @g, i32 126)
  ret i16 %v
}

define void @array_gep() {
  store i32 7, ptr getelementptr inbounds ([16 x i32], ptr @arr, i32 0, i32 3)
  ret void
}

define void @cast_expr() {
  store i32 ptrtoint (ptr @g to i32), ptr @p
  ret void
}
