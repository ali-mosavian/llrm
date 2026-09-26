; ON TIMER: at each check BC makes, a call to @llrm.qb.events, which
; names the handler to run, 0 for none; the handler returns through the
; continuation index as GOSUB does. The model fires handler 1 once.
; expect: 5
target datalayout = "e-p:16:16-p1:32:16:16:16-i32:16-i64:16"

@fired = internal global i1 false

define i16 @llrm.qb.events() {
  %was = load i1, ptr @fired
  store i1 true, ptr @fired
  %h = select i1 %was, i16 0, i16 1
  ret i16 %h
}

define i32 @main() {
entry:
  %count = alloca i16, align 2
  store i16 0, ptr %count, align 2
  br label %check0
check0:
  %h0 = call i16 @llrm.qb.events()
  switch i16 %h0, label %stmt1 [ i16 1, label %handler ]
stmt1:
  %h1 = call i16 @llrm.qb.events()
  switch i16 %h1, label %done [ i16 1, label %handler ]
handler:
  %from = phi i16 [ 0, %check0 ], [ 1, %stmt1 ]
  %c = load i16, ptr %count, align 2
  %c5 = add i16 %c, 5
  store i16 %c5, ptr %count, align 2
  br label %resume
resume:
  switch i16 %from, label %lost [ i16 0, label %stmt1
                                  i16 1, label %done ]
done:
  %r = load i16, ptr %count, align 2
  %w = zext i16 %r to i32
  ret i32 %w
lost:
  unreachable
}
