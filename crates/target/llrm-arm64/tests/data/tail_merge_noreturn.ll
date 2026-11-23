; Four blocks that each end in `ok(c, N); syn(c, 1); unreachable`: the pin (BranchFolder.TailMergeBlocks takes the blocks with no successor) shares the calls, 3 bl against our 8.
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
declare void @syn(ptr, i32) noreturn
declare void @ok(ptr, i32)
define void @t(i32 %k, ptr %c) optsize nounwind {
entry:
  switch i32 %k, label %d [ i32 0, label %a
                            i32 1, label %b
                            i32 2, label %e
                            i32 3, label %f ]
a:
  call void @ok(ptr %c, i32 7)
  call void @syn(ptr %c, i32 1)
  unreachable
b:
  call void @ok(ptr %c, i32 8)
  call void @syn(ptr %c, i32 1)
  unreachable
e:
  call void @ok(ptr %c, i32 9)
  call void @syn(ptr %c, i32 1)
  unreachable
f:
  call void @ok(ptr %c, i32 5)
  call void @syn(ptr %c, i32 1)
  unreachable
d:
  ret void
}
