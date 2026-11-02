target triple = "wasm32-unknown-unknown"
define i32 @ld(ptr %p) { %v = load i32, ptr %p
 ret i32 %v }
define void @st(ptr %p, i32 %x) { store i32 %x, ptr %p
 ret void }
define i32 @addr() { %v = load i32, ptr inttoptr (i32 64 to ptr)
 ret i32 %v }
define i32 @sum3(i32 %a, i32 %b, i32 %c) { %x = add i32 %a, %b
 %y = add i32 %x, %c
 ret i32 %y }
