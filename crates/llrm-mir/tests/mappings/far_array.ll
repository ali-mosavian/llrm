; A far array: a pointer in address space 1, indexed by 16 bits.
; expect: 60
target datalayout = "e-p:16:16-p1:32:16:16:16-i32:16-i64:16"

@data = internal addrspace(1) global [3 x i16] [i16 10, i16 20, i16 30], align 2

define i16 @sum(ptr addrspace(1) %base, i16 %n) {
entry:
  br label %head
head:
  %i = phi i16 [ 0, %entry ], [ %next, %body ]
  %total = phi i16 [ 0, %entry ], [ %added, %body ]
  %more = icmp slt i16 %i, %n
  br i1 %more, label %body, label %done
body:
  %at = getelementptr inbounds i16, ptr addrspace(1) %base, i16 %i
  %x = load i16, ptr addrspace(1) %at, align 2
  %added = add i16 %total, %x
  %next = add nsw i16 %i, 1
  br label %head
done:
  ret i16 %total
}

define i32 @main() {
  %s = call i16 @sum(ptr addrspace(1) @data, i16 3)
  %w = zext i16 %s to i32
  ret i32 %w
}
