; A self loop, a loop with two latches, a loop whose exit is named by two
; edges of one branch, a loop with two exits to one block through a switch,
; a loop that never exits, and a cycle no entry reaches.
define void @self(i1 %p) {
entry:
  br label %l
l:
  br i1 %p, label %l, label %exit
exit:
  ret void
}

define void @two_latches(i1 %p, i1 %q) {
entry:
  br label %h
h:
  br i1 %p, label %a, label %b
a:
  br i1 %q, label %h, label %exit
b:
  br label %h
exit:
  ret void
}

define void @repeated_exit(i1 %p) {
entry:
  br label %h
h:
  br i1 %p, label %h, label %exit
exit:
  br i1 %p, label %x, label %x
x:
  ret void
}

define void @switch_exits(i32 %n) {
entry:
  br label %h
h:
  switch i32 %n, label %exit [
    i32 0, label %h
    i32 1, label %exit
    i32 2, label %body
  ]
body:
  br label %h
exit:
  ret void
}

define void @forever() {
entry:
  br label %l
l:
  br label %l
}

define void @unreachable_cycle(i1 %p) {
entry:
  ret void
dead1:
  br label %dead2
dead2:
  br i1 %p, label %dead1, label %dead3
dead3:
  ret void
}

define void @nest(i1 %p, i1 %q) {
entry:
  br label %o
o:
  br label %i
i:
  br i1 %q, label %i, label %ol
ol:
  br i1 %p, label %o, label %j
j:
  br label %k
k:
  br i1 %q, label %k, label %jl
jl:
  br i1 %p, label %j, label %exit
exit:
  ret void
}
