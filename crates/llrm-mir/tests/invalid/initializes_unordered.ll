; invalid: Attribute 'initializes' does not support unordered ranges
declare void @f(ptr initializes((4, 8), (0, 2)))
