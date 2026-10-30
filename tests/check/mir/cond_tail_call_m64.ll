target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-unknown-linux-gnu"
@free_p = external global ptr
@end_p = external global ptr
declare void @other()
define void @take() {
  %a = load ptr, ptr @free_p
  %b = load ptr, ptr @end_p
  %c = icmp ult ptr %a, %b
  br i1 %c, label %t, label %done
t:
  %v = load i32, ptr %a
  %m = and i32 %v, 1
  %z = icmp eq i32 %m, 0
  br i1 %z, label %call, label %chk
chk:
  %q = getelementptr i8, ptr %a, i32 7
  %lt = icmp ult ptr %q, %b
  br i1 %lt, label %call, label %mid
call:
  tail call void @other()
  br label %mid
mid:
  br label %done
done:
  ret void
}
