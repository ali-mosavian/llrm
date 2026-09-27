; invalid: Global is external, but doesn't have external or weak linkage!
declare internal void @f()

define void @g() {
  call void @f()
  ret void
}
