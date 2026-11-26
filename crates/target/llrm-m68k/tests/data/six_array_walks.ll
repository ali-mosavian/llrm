target triple = "m68k-unknown-linux-gnu"
target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

@px = global [100 x i16] zeroinitializer, align 2
@py = global [100 x i16] zeroinitializer, align 2
@pz = global [100 x i16] zeroinitializer, align 2
@vx = global [100 x i16] zeroinitializer, align 2
@vy = global [100 x i16] zeroinitializer, align 2
@vz = global [100 x i16] zeroinitializer, align 2

define void @advance() {
  br label %loop

loop:
  %i = phi i32 [ 0, %0 ], [ %next, %loop ]
  %px.at = getelementptr inbounds nuw [2 x i8], ptr @px, i32 %i
  %px.old = load i16, ptr %px.at, align 2
  %vx.at = getelementptr inbounds nuw [2 x i8], ptr @vx, i32 %i
  %vx.old = load i16, ptr %vx.at, align 2
  %px.new = add i16 %px.old, %vx.old
  store i16 %px.new, ptr %px.at, align 2
  %py.at = getelementptr inbounds nuw [2 x i8], ptr @py, i32 %i
  %py.old = load i16, ptr %py.at, align 2
  %vy.at = getelementptr inbounds nuw [2 x i8], ptr @vy, i32 %i
  %vy.old = load i16, ptr %vy.at, align 2
  %py.new = add i16 %py.old, %vy.old
  store i16 %py.new, ptr %py.at, align 2
  %pz.at = getelementptr inbounds nuw [2 x i8], ptr @pz, i32 %i
  %pz.old = load i16, ptr %pz.at, align 2
  %vz.at = getelementptr inbounds nuw [2 x i8], ptr @vz, i32 %i
  %vz.old = load i16, ptr %vz.at, align 2
  %pz.new = add i16 %pz.old, %vz.old
  store i16 %pz.new, ptr %pz.at, align 2
  %next = add nuw nsw i32 %i, 1
  %done = icmp eq i32 %next, 100
  br i1 %done, label %exit, label %loop

exit:
  ret void
}
