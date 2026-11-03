target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"
define weak void @f() { ret void }
define linkonce_odr void @g() { ret void }
define internal void @h() { ret void }
define void @user() { call void @f()  call void @g() call void @h() ret void }
