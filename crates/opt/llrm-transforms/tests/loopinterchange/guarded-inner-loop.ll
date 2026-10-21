; ModuleID = '<stdin>'
source_filename = "<stdin>"

@x = global [3 x [3 x [3 x i32]]] zeroinitializer
@w = global [3 x [3 x [3 x i32]]] zeroinitializer
@y = global [3 x [3 x [3 x i32]]] zeroinitializer

define i32 @main() {
entry:
  br label %for.i

for.i:                                            ; preds = %for.i.inc, %entry
  %i = phi i32 [ %i.next, %for.i.inc ], [ 0, %entry ]
  %i.is.zero = icmp eq i32 %i, 0
  %xbase = getelementptr [9 x i32], ptr @x, i32 %i
  %wbase = getelementptr [9 x i32], ptr @w, i32 %i
  %ybase = getelementptr [3 x i32], ptr @y, i32 %i
  br label %for.j

for.j:                                            ; preds = %for.j.inc, %for.i
  %j = phi i32 [ %j.next, %for.j.inc ], [ 0, %for.i ]
  br i1 %i.is.zero, label %for.j.inc, label %for.k.ph

for.k.ph:                                         ; preds = %for.j
  %xp = getelementptr i32, ptr %xbase, i32 %j
  %wp = getelementptr i32, ptr %wbase, i32 %j
  %yp = getelementptr [9 x i32], ptr %ybase, i32 %j
  br label %for.k

for.k:                                            ; preds = %for.k, %for.k.ph
  %k = phi i32 [ 0, %for.k.ph ], [ %k.next, %for.k ]
  %xk = getelementptr [3 x i32], ptr %xp, i32 %k
  %xv = load i32, ptr %xk, align 4
  %wk = getelementptr [3 x i32], ptr %wp, i32 %k
  %wv = load i32, ptr %wk, align 4
  %add = add i32 %xv, %wv
  %yk = getelementptr i32, ptr %yp, i32 %k
  store i32 %add, ptr %yk, align 4
  %k.next = add i32 %k, 1
  %k.done = icmp eq i32 %k.next, %i
  br i1 %k.done, label %for.j.inc.loopexit, label %for.k

for.j.inc.loopexit:                               ; preds = %for.k
  br label %for.j.inc

for.j.inc:                                        ; preds = %for.j.inc.loopexit, %for.j
  %j.next = add i32 %j, 1
  %j.cmp = icmp eq i32 %j, 0
  br i1 %j.cmp, label %for.j, label %for.i.inc

for.i.inc:                                        ; preds = %for.j.inc
  %i.next = add i32 %i, 1
  %i.done = icmp eq i32 %i.next, 3
  br i1 %i.done, label %exit, label %for.i

exit:                                             ; preds = %for.i.inc
  ret i32 0
}
