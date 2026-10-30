%S = type { i8, i16 }
%T = type { [0 x i8], i8 }
%E = type {}
@g = global i32 0

define void @ints(ptr %p, i8 %b, i32 %x) {
  store i8 %b, ptr %p
  store i8 7, ptr %p
  store i8 poison, ptr %p
  store i32 -1, ptr %p
  store i32 0, ptr %p
  store i32 16843009, ptr %p
  store i32 1, ptr %p
  store i32 %x, ptr %p
  store i16 -1, ptr %p
  store i16 257, ptr %p
  store i16 258, ptr %p
  store i64 -1, ptr %p
  store i64 72340172838076673, ptr %p
  store i64 72340172838076674, ptr %p
  store i1 true, ptr %p
  store i1 false, ptr %p
  store i24 -1, ptr %p
  store i128 -1, ptr %p
  store i128 1, ptr %p
  store i32 poison, ptr %p
  store i64 poison, ptr %p
  ret void
}

define void @floats(ptr %p) {
  store float 0.0, ptr %p
  store float -0.0, ptr %p
  store float 1.0, ptr %p
  store double 0.0, ptr %p
  store double -0.0, ptr %p
  store double 1.0, ptr %p
  store double 0x8080808080808080, ptr %p
  store double 0x0101010101010101, ptr %p
  store x86_fp80 0.0, ptr %p
  store x86_fp80 1.0, ptr %p
  ret void
}

define void @pointers(ptr %p) {
  store ptr null, ptr %p
  store ptr inttoptr (i64 0 to ptr), ptr %p
  store ptr inttoptr (i64 -1 to ptr), ptr %p
  store ptr inttoptr (i64 4 to ptr), ptr %p
  store ptr inttoptr (i32 -1 to ptr), ptr %p
  store ptr inttoptr (i128 -1 to ptr), ptr %p
  store ptr @g, ptr %p
  store ptr getelementptr (i8, ptr @g, i64 1), ptr %p
  ret void
}

define void @aggregates(ptr %p) {
  store [4 x i8] zeroinitializer, ptr %p
  store [4 x i8] c"aaaa", ptr %p
  store [3 x i8] c"abc", ptr %p
  store [2 x i16] [i16 -1, i16 -1], ptr %p
  store [2 x i16] [i16 -1, i16 0], ptr %p
  store [2 x i16] [i16 257, i16 257], ptr %p
  store [2 x i16] [i16 poison, i16 257], ptr %p
  store [2 x i16] [i16 poison, i16 poison], ptr %p
  store [2 x i16] [i16 poison, i16 258], ptr %p
  store { i8, i16 } { i8 1, i16 257 }, ptr %p
  store { i8, i16 } { i8 1, i16 2 }, ptr %p
  store { i8, i16 } zeroinitializer, ptr %p
  store [0 x i8] zeroinitializer, ptr %p
  store {} zeroinitializer, ptr %p
  store [2 x [0 x i8]] zeroinitializer, ptr %p
  store { [0 x i8], i8 } { [0 x i8] zeroinitializer, i8 5 }, ptr %p
  store { [0 x i8], i8 } zeroinitializer, ptr %p
  store [2 x float] [float 0.0, float 0.0], ptr %p
  store [2 x ptr] [ptr null, ptr null], ptr %p
  store [2 x ptr] [ptr null, ptr @g], ptr %p
  ret void
}
