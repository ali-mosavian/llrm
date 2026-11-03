target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"
declare void @use(ptr, ptr)
define void @f(i16 %n) {
  %a = alloca [16 x i16], align 8
  %b = alloca [16 x i16], align 8
  %v0 = add i16 %n, 0
  %a0 = getelementptr inbounds i16, ptr %a, i32 0
  %b0 = getelementptr inbounds i16, ptr %b, i32 0
  store i16 %v0, ptr %a0, align 2
  store i16 1, ptr %b0, align 2
  %v1 = add i16 %n, 1
  %a1 = getelementptr inbounds i16, ptr %a, i32 1
  %b1 = getelementptr inbounds i16, ptr %b, i32 1
  store i16 %v1, ptr %a1, align 2
  store i16 2, ptr %b1, align 2
  %v2 = add i16 %n, 2
  %a2 = getelementptr inbounds i16, ptr %a, i32 2
  %b2 = getelementptr inbounds i16, ptr %b, i32 2
  store i16 %v2, ptr %a2, align 2
  store i16 3, ptr %b2, align 2
  %v3 = add i16 %n, 3
  %a3 = getelementptr inbounds i16, ptr %a, i32 3
  %b3 = getelementptr inbounds i16, ptr %b, i32 3
  store i16 %v3, ptr %a3, align 2
  store i16 4, ptr %b3, align 2
  %v4 = add i16 %n, 4
  %a4 = getelementptr inbounds i16, ptr %a, i32 4
  %b4 = getelementptr inbounds i16, ptr %b, i32 4
  store i16 %v4, ptr %a4, align 2
  store i16 5, ptr %b4, align 2
  %v5 = add i16 %n, 5
  %a5 = getelementptr inbounds i16, ptr %a, i32 5
  %b5 = getelementptr inbounds i16, ptr %b, i32 5
  store i16 %v5, ptr %a5, align 2
  store i16 6, ptr %b5, align 2
  %v6 = add i16 %n, 6
  %a6 = getelementptr inbounds i16, ptr %a, i32 6
  %b6 = getelementptr inbounds i16, ptr %b, i32 6
  store i16 %v6, ptr %a6, align 2
  store i16 7, ptr %b6, align 2
  %v7 = add i16 %n, 7
  %a7 = getelementptr inbounds i16, ptr %a, i32 7
  %b7 = getelementptr inbounds i16, ptr %b, i32 7
  store i16 %v7, ptr %a7, align 2
  store i16 8, ptr %b7, align 2
  %v8 = add i16 %n, 8
  %a8 = getelementptr inbounds i16, ptr %a, i32 8
  %b8 = getelementptr inbounds i16, ptr %b, i32 8
  store i16 %v8, ptr %a8, align 2
  store i16 9, ptr %b8, align 2
  %v9 = add i16 %n, 9
  %a9 = getelementptr inbounds i16, ptr %a, i32 9
  %b9 = getelementptr inbounds i16, ptr %b, i32 9
  store i16 %v9, ptr %a9, align 2
  store i16 10, ptr %b9, align 2
  %v10 = add i16 %n, 10
  %a10 = getelementptr inbounds i16, ptr %a, i32 10
  %b10 = getelementptr inbounds i16, ptr %b, i32 10
  store i16 %v10, ptr %a10, align 2
  store i16 11, ptr %b10, align 2
  %v11 = add i16 %n, 11
  %a11 = getelementptr inbounds i16, ptr %a, i32 11
  %b11 = getelementptr inbounds i16, ptr %b, i32 11
  store i16 %v11, ptr %a11, align 2
  store i16 12, ptr %b11, align 2
  %v12 = add i16 %n, 12
  %a12 = getelementptr inbounds i16, ptr %a, i32 12
  %b12 = getelementptr inbounds i16, ptr %b, i32 12
  store i16 %v12, ptr %a12, align 2
  store i16 13, ptr %b12, align 2
  %v13 = add i16 %n, 13
  %a13 = getelementptr inbounds i16, ptr %a, i32 13
  %b13 = getelementptr inbounds i16, ptr %b, i32 13
  store i16 %v13, ptr %a13, align 2
  store i16 14, ptr %b13, align 2
  %v14 = add i16 %n, 14
  %a14 = getelementptr inbounds i16, ptr %a, i32 14
  %b14 = getelementptr inbounds i16, ptr %b, i32 14
  store i16 %v14, ptr %a14, align 2
  store i16 15, ptr %b14, align 2
  %v15 = add i16 %n, 15
  %a15 = getelementptr inbounds i16, ptr %a, i32 15
  %b15 = getelementptr inbounds i16, ptr %b, i32 15
  store i16 %v15, ptr %a15, align 2
  store i16 16, ptr %b15, align 2
  call void @use(ptr %a, ptr %b)
  ret void
}
