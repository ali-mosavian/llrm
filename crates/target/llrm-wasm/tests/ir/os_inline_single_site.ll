target triple = "wasm32-unknown-unknown"

define i32 @helper(i32 %a, i32 %b) {
  %t0 = add i32 %a, %b
  %t1 = mul i32 %t0, %a
  %t2 = xor i32 %t1, %b
  %t3 = sub i32 %t2, %a
  %t4 = or i32 %t3, %b
  %t5 = and i32 %t4, %a
  %t6 = add i32 %t5, %b
  %t7 = xor i32 %t6, %a
  %t8 = mul i32 %t7, %b
  %t9 = sub i32 %t8, %a
  ret i32 %t9
}

define i32 @user(i32 %x, i32 %y) optsize {
  %r = call i32 @helper(i32 %x, i32 %y)
  ret i32 %r
}
