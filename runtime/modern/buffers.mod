# Strings' and vecs' buffers (section 13): a descriptor in front of the
# data, [flags][pad][length][capacity], and a NUL after it.

import errors
import heap

pub const HEAP: u8 = 1
pub const READONLY: u8 = 8
const DESCRIPTOR: u16 = 6
# The most a buffer's descriptor, data and NUL take.
const LARGEST: u16 = 0xFFF0

pub fn flags(data: *near mut u8) -> *near mut u8:
    return data.offset(-6)

pub fn length(data: *near mut u8) -> *near mut u16:
    return data.offset(-4).cast[u16]()

pub fn capacity(data: *near mut u8) -> *near mut u16:
    return data.offset(-2).cast[u16]()

pub fn copy(to: *near mut u8, source: *far u8, count: u16) -> void:
    unsafe:
        for at in range(0, count):
            to[at] = source[at]

# Whether `capacity` elements of `size` bytes fit in one buffer.
fn fits(capacity: u16, size: u16) -> bool:
    return size == 0 || capacity <= (LARGEST - DESCRIPTOR - 1) // size

# An empty heap buffer for `capacity` elements of `size` bytes.
pub fn allocate(capacity: u16, size: u16) -> *near mut u8:
    if !fits(capacity, size):
        errors.panic("out of memory")
    let data = heap.allocate(capacity * size + DESCRIPTOR + 1).offset(6)
    unsafe:
        *flags(data) = HEAP
        *flags(data).offset(1) = 0
        *length(data) = 0
        *capacity(data) = capacity
        *data = 0
    return data

export "cdecl16":
    # `data` on the heap and writable, with room for `capacity` elements:
    # the same buffer when it already is, else a copy, and the old one
    # dropped. Room at least doubles, so appending one at a time is linear.
    @link_name("M$BRES")
    pub fn reserve(data: *near mut u8, capacity: u16, size: u16) -> *near mut u8:
        unsafe:
            let length = *length(data)
            let room = *capacity(data)
            let wanted = capacity < length ? length : capacity
            let owned = *flags(data) & (HEAP | READONLY) == HEAP
            if owned && room >= wanted:
                return data
            let doubled = room < 0x8000 ? room * 2 : room
            let target = (wanted < doubled && fits(doubled, size)) ? doubled : wanted
            if owned && fits(target, size) && heap.resize(data.offset(-6), target * size + DESCRIPTOR + 1):
                *capacity(data) = target
                return data
            let moved = allocate(target, size)
            copy(moved, data.far(), length * size + 1)
            *length(moved) = length
            drop(data)
            return moved

    @link_name("M$BDRP")
    pub fn drop(data: *near mut u8) -> void:
        unsafe:
            if !data.is_null() && *flags(data) & HEAP != 0:
                heap.release(data.offset(-6))

    # `data` with `count` more elements counted in its length; it may move.
    @link_name("M$BGRW")
    fn grow(data: *near mut u8, count: u16, size: u16) -> *near mut u8:
        unsafe:
            let length = *length(data)
            let grown = reserve(data, length + count, size)
            *length(grown) = length + count
            return grown

    # `data` without its last `count` elements; the new length.
    @link_name("M$BSHR")
    fn shrink(data: *near mut u8, count: u16) -> u16:
        unsafe:
            let length = *length(data)
            if length < count:
                errors.panic("pop from an empty vec")
            *length(data) = length - count
            return length - count

    # A heap copy of `data`.
    @link_name("M$BCLN")
    fn clone(data: *near mut u8, size: u16) -> *near mut u8:
        unsafe:
            let length = *length(data)
            let copied = allocate(length, size)
            copy(copied, data.far(), length * size + 1)
            *length(copied) = length
            return copied
