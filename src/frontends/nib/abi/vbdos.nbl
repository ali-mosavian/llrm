# VB-DOS's side of `export "vbdos":` and `extern "vbdos":` (section
# 15). BASIC passes an argument by reference: a near pointer to the
# variable, or to its string or array descriptor. Each adapter is that
# pointer, and borrows what it points to.

import abi.basic

# `name AS T`, borrowed as `&mut T`.
pub struct Ref[T]:
    target: *near mut T

# `name$`, its characters borrowed as `&mut [char]`.
pub struct StringRef:
    descriptor: *near StringDescriptor

# `name() AS T`, its elements borrowed as `&mut [T, N]`.
pub struct ArrayRef[T]:
    descriptor: *near basic.ArrayDescriptor

# A far string, which only the runtime reads (PDS 7.1 Programmer's Guide,
# "StringAddress, StringAssign, StringLength, and StringRelease").
@repr("c16", pack=1)
pub struct StringDescriptor:
    mut first: u16
    mut second: u16

extern "pascal16":
    @link_name("STRINGADDRESS")
    pub fn string_data(text: *near StringDescriptor) -> *far mut char
    @link_name("STRINGLENGTH")
    pub fn string_length(text: *near StringDescriptor) -> u16
    @link_name("STRINGASSIGN")
    fn assign(data: *far mut char, length: u16, target: *far mut StringDescriptor, target_length: u16) -> void
    # A copy of a string on BASIC's temporaries.
    @link_name("B$SCPY")
    fn copied(text: *near StringDescriptor) -> *near StringDescriptor

var result: StringDescriptor = StringDescriptor(first=0, second=0)

# A string function's result: `data` copied where BASIC takes it from.
pub fn string_result(data: *far mut char, length: u16) -> *near StringDescriptor:
    unsafe:
        assign(data, length, &mut result, 0)
        return copied(&result)
