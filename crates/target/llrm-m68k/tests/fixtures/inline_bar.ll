target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
@g = global i32 0
define internal void @h(i32 %v0) {
  %v1 = mul i32 %v0, 4
  %v2 = add i32 %v1, 7
  %v3 = xor i32 %v2, 12
  %v4 = mul i32 %v3, 11
  %v5 = add i32 %v4, 16
  %v6 = xor i32 %v5, 19
  %v7 = mul i32 %v6, 10
  %v8 = add i32 %v7, 13
  %v9 = xor i32 %v8, 18
  %v10 = mul i32 %v9, 17
  %v11 = add i32 %v10, 22
  %v12 = xor i32 %v11, 25
  %v13 = mul i32 %v12, 16
  %v14 = add i32 %v13, 19
  %v15 = xor i32 %v14, 24
  %v16 = mul i32 %v15, 23
  %v17 = add i32 %v16, 28
  %v18 = xor i32 %v17, 31
  %v19 = mul i32 %v18, 22
  %v20 = add i32 %v19, 25
  %v21 = xor i32 %v20, 30
  %v22 = mul i32 %v21, 29
  %v23 = add i32 %v22, 34
  %v24 = xor i32 %v23, 37
  %v25 = mul i32 %v24, 28
  %v26 = add i32 %v25, 31
  %v27 = xor i32 %v26, 36
  %v28 = mul i32 %v27, 35
  %v29 = add i32 %v28, 40
  %v30 = xor i32 %v29, 43
  store volatile i32 %v30, ptr @g
  ret void
}
define void @k(i32 %a0, i32 %a1, i32 %a2, i32 %a3, i32 %a4, i32 %a5) {
  call void @h(i32 %a0)
  call void @h(i32 %a1)
  call void @h(i32 %a2)
  call void @h(i32 %a3)
  call void @h(i32 %a4)
  call void @h(i32 %a5)
  ret void
}
