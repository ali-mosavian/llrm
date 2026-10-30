# What QuickBASIC 4.5, PDS 7.1 and VB-DOS share (section 15): the
# descriptor BASIC passes for an array argument, runtime array.inc's AD.

@repr("c16", pack=1)
pub struct ArrayDescriptor:
    # The first element.
    data: *far mut u8
    next: u16
    paragraphs: u16
    rank: u8
    features: u8
    adjusted: u16
    element_bytes: u16
    # A (count, lower bound) word pair per dimension follows, last dimension first.

pub fn array_data(array: *near ArrayDescriptor) -> *far mut u8:
    unsafe:
        return (*array).data

# The element count of `dimension`, in the descriptor's order.
pub fn array_count(array: *near ArrayDescriptor, dimension: u16) -> u16:
    unsafe:
        return array.offset(1).cast[u16]()[2 * dimension]
