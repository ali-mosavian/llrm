; invalid: the entry block has predecessors
define void @f() {
entry:
  br label %entry
}
