target datalayout = "e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32"
target triple = "i486-unknown-linux-gnu"
declare void @other()
define void @take(i32 %a, i32 %b) {
  %c = icmp ult i32 %a, %b
  br i1 %c, label %t, label %done
t:
  %m = and i32 %a, 1
  %z = icmp eq i32 %m, 0
  br i1 %z, label %call, label %chk
chk:
  %q = add i32 %a, 7
  %lt = icmp ult i32 %q, %b
  br i1 %lt, label %call, label %done
call:
  tail call void @other()
  ret void
done:
  ret void
}
