target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"
target triple = "i386-unknown-unknown-code16"
declare void @other()
define void @take(i16 %a, i16 %b) {
  %c = icmp ult i16 %a, %b
  br i1 %c, label %t, label %done
t:
  %m = and i16 %a, 1
  %z = icmp eq i16 %m, 0
  br i1 %z, label %call, label %chk
chk:
  %q = add i16 %a, 7
  %lt = icmp ult i16 %q, %b
  br i1 %lt, label %call, label %done
call:
  tail call void @other()
  ret void
done:
  ret void
}
