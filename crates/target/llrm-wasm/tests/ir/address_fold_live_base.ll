target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"

; The base is read again in the block that loads: the address folds there.
define i32 @live(ptr %p, ptr %q, i1 %c) {
  %a = getelementptr inbounds nuw i8, ptr %p, i32 8
  store ptr %a, ptr %q, align 4
  br i1 %c, label %t, label %e
t:
  %b = getelementptr inbounds nuw i8, ptr %p, i32 12
  store ptr %b, ptr %q, align 4
  %v = load i32, ptr %a, align 4
  ret i32 %v
e:
  ret i32 0
}

; The base is read nowhere in the block: the shared address stays.
define i32 @dead(ptr %p, ptr %q, i1 %c) {
  %a = getelementptr inbounds nuw i8, ptr %p, i32 8
  store ptr %a, ptr %q, align 4
  br i1 %c, label %t, label %e
t:
  %v = load i32, ptr %a, align 4
  ret i32 %v
e:
  ret i32 0
}
