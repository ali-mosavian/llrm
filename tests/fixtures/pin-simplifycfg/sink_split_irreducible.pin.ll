; output of the pin's opt on sink_split_irreducible.in.ll: H.sink.split has a pred outside the loop (P1) and the latch (L): irreducible
; ModuleID = 'b.ll'
source_filename = "b.ll"

declare i32 @rd()

declare ptr @bg(ptr, i32, i32)

declare i32 @cnd()

define void @f(ptr %p0, i32 %k) {
entry:
  %e = call i32 @cnd()
  %ec = icmp eq i32 %e, 0
  br i1 %ec, label %P1, label %common.ret

P1:                                               ; preds = %entry
  %cond = icmp eq i32 %k, 13
  br i1 %cond, label %H, label %H.sink.split

H.sink.split:                                     ; preds = %P1, %L
  %ph.sink = phi ptr [ %ph, %L ], [ %p0, %P1 ]
  %gl = call ptr @bg(ptr %ph.sink, i32 1, i32 1)
  br label %H

H:                                                ; preds = %H.sink.split, %P1, %L
  %ph = phi ptr [ %p0, %P1 ], [ %ph, %L ], [ %gl, %H.sink.split ]
  %v = call i32 @rd()
  %c = icmp slt i32 %v, 0
  br i1 %c, label %common.ret, label %L

L:                                                ; preds = %H
  switch i32 %v, label %H.sink.split [
    i32 10, label %common.ret
    i32 13, label %H
  ]

common.ret:                                       ; preds = %entry, %H, %L
  ret void
}
