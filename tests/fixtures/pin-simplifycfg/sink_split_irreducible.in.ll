; opt -passes='simplifycfg<sink-common-insts>' (pin d1106deb71cc): input
declare i32 @rd()
declare ptr @bg(ptr, i32, i32)
declare i32 @cnd()
define void @f(ptr %p0, i32 %k) {
entry:
  %e = call i32 @cnd()
  %ec = icmp eq i32 %e, 0
  br i1 %ec, label %P1, label %exit
P1:
  switch i32 %k, label %PC [ i32 13, label %H ]
PC:
  %g1 = call ptr @bg(ptr %p0, i32 1, i32 1)
  br label %H
H:
  %ph = phi ptr [ %p0, %P1 ], [ %g1, %PC ], [ %gl, %LC ], [ %ph, %L ]
  %v = call i32 @rd()
  %c = icmp slt i32 %v, 0
  br i1 %c, label %exit, label %L
L:
  switch i32 %v, label %LC [ i32 10, label %exit2
                             i32 13, label %H ]
LC:
  %gl = call ptr @bg(ptr %ph, i32 1, i32 1)
  br label %H
exit2:
  ret void
exit:
  ret void
}
