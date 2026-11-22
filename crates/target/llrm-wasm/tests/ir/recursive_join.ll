target triple = "wasm32-unknown-unknown"
target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"

define i32 @deep(i32 %n, i32 %a, i32 %b, i32 %c, i32 %d) {
entry:
  %z = icmp eq i32 %n, 0
  br i1 %z, label %base, label %rec
base:
  %ab = add i32 %a, %b
  %abc = add i32 %ab, %c
  br label %join
rec:
  %n1 = sub i32 %n, 1
  %an = add i32 %a, %n
  %r = call i32 @deep(i32 %n1, i32 %b, i32 %c, i32 %d, i32 %an)
  br label %join
join:
  %t = phi i32 [ %abc, %base ], [ %r, %rec ]
  %s = phi i32 [ %d, %base ], [ %n, %rec ]
  %res = add i32 %t, %s
  ret i32 %res
}
