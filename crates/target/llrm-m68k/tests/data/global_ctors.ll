target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"
@llvm.global_ctors = appending global [2 x { i32, ptr, ptr }] [{ i32, ptr, ptr } { i32 65535, ptr @late, ptr null }, { i32, ptr, ptr } { i32 101, ptr @early, ptr null }]
@count = global i32 0, align 4
define internal void @late() { store i32 2, ptr @count, align 4 ret void }
define internal void @early() { store i32 1, ptr @count, align 4 ret void }
