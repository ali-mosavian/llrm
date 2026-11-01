define i32 @whiles(i32 %n) {
entry:
  br label %cond
cond:
  %s = phi i32 [ 0, %entry ], [ %add, %body ]
  %i = phi i32 [ %n, %entry ], [ %dec, %body ]
  %c = icmp sgt i32 %i, 0
  br i1 %c, label %body, label %end
body:
  %add = add i32 %s, %i
  %dec = add i32 %i, -1
  br label %cond
end:
  ret i32 %s
}
