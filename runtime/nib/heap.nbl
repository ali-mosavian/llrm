# The near heap: DGROUP after the stack, taken from DOS as it is needed.
# Free blocks wait in one list per power-of-two size class and merge with
# free neighbours when freed, so the heap does not fragment into slivers.
#
# A block is a header word, then its payload. The header is the block's
# size, a multiple of 4 counting the header, with two flags in its low
# bits: USED, and BEFORE_USED when the block in front is in use. A free
# block keeps its list links after the header and its size in its last
# word, where the block after it finds its start. A used, empty block ends
# the heap.

import errors
import os

const USED: u16 = 1
const BEFORE_USED: u16 = 2
const FLAGS: u16 = 3
const SMALLEST: u16 = 8
const CLASSES = 13
# Growth below this many bytes asks DOS again too often.
const GROWTH: u16 = 1024

var heads: *near mut u8[CLASSES] = [0] * CLASSES
# Bit c is set while list c holds a block.
var occupied: u16 = 0
var end: *near mut u8 = 0

fn header(block: *near mut u8) -> *near mut u16:
    return block.cast[u16]()

fn size_of(block: *near mut u8) -> u16:
    unsafe:
        return *header(block) & ~FLAGS

fn next(block: *near mut u8) -> *near mut *near mut u8:
    return block.offset(2).cast[*near mut u8]()

fn previous(block: *near mut u8) -> *near mut *near mut u8:
    return block.offset(4).cast[*near mut u8]()

# floor(log2(size)) - 3: 8 to 15 bytes are class 0.
fn class_of(size: u16) -> u16:
    let mut class: u16 = 0
    let mut rest = size >> 4
    while rest != 0:
        class += 1
        rest >>= 1
    return class

# A block of `bytes` payload, and its header.
fn rounded(bytes: u16) -> u16:
    if bytes > 0xFFF0:
        errors.panic("out of memory")
    let size = (bytes + 5) & ~FLAGS
    return size > SMALLEST ? size : SMALLEST

fn unlink(block: *near mut u8) -> void:
    unsafe:
        let after = *next(block)
        let before = *previous(block)
        if before.is_null():
            let class = class_of(size_of(block))
            heads[class] = after
            if after.is_null():
                occupied &= ~(u16(1) << class)
        else:
            *next(before) = after
        if !after.is_null():
            *previous(after) = before

# `block` free with `size` bytes: listed, its size in its last word, and
# the block after it told.
fn insert(block: *near mut u8, size: u16) -> void:
    unsafe:
        *header(block) = size | (*header(block) & BEFORE_USED)
        *header(block.offset(size - 2)) = size
        *header(block.offset(size)) &= ~BEFORE_USED
        let class = class_of(size)
        let first = heads[class]
        *next(block) = first
        *previous(block) = 0
        if !first.is_null():
            *previous(first) = block
        heads[class] = block
        occupied |= u16(1) << class

# A free block of at least `size` bytes, or null: the first that fits in
# its own class, else the first of the next class that holds one, where
# any fits.
fn fit(size: u16) -> *near mut u8:
    let mut class = class_of(size)
    unsafe:
        let mut block = heads[class]
        while !block.is_null():
            if size_of(block) >= size:
                return block
            block = *next(block)
    let mut larger = occupied & ~((u16(2) << class) - 1)
    if larger == 0:
        return 0
    class = 0
    while larger & 1 == 0:
        larger >>= 1
        class += 1
    return heads[class]

# The heap `size` bytes longer, or not when DOS or DGROUP has no more.
fn grow(size: u16) -> void:
    let mut wanted = size > GROWTH ? size : GROWTH
    unsafe:
        # The first grant also holds the end block; later ones reuse it.
        let first = end.is_null()
        let extra: u16 = first ? 2 : 0
        let mut more = os.more(wanted + extra)
        if more.is_null():
            wanted = size
            more = os.more(wanted + extra)
            if more.is_null():
                return
        let block = first ? more : end
        if first:
            *header(block) = BEFORE_USED
        end = block.offset(wanted)
        *header(end) = USED | BEFORE_USED
        *header(block) = wanted | USED | (*header(block) & BEFORE_USED)
        release(block.offset(2))

# Frees all of `block` past `size` bytes, when that is a block's worth.
fn trim(block: *near mut u8, size: u16) -> void:
    let total = size_of(block)
    if total - size < SMALLEST:
        return
    unsafe:
        let rest = block.offset(size)
        *header(rest) = (total - size) | USED | BEFORE_USED
        *header(block) = size | (*header(block) & FLAGS)
        release(rest.offset(2))

# `bytes` of payload, word aligned.
pub fn allocate(bytes: u16) -> *near mut u8:
    let size = rounded(bytes)
    let mut block = fit(size)
    if block.is_null():
        grow(size)
        block = fit(size)
        if block.is_null():
            errors.panic("out of memory")
    unlink(block)
    unsafe:
        let whole = size_of(block)
        *header(block) = whole | USED | (*header(block) & BEFORE_USED)
        *header(block.offset(whole)) |= BEFORE_USED
    trim(block, size)
    return block.offset(2)

pub fn release(payload: *near mut u8) -> void:
    if payload.is_null():
        return
    let mut block = payload.offset(-2)
    let mut size = size_of(block)
    unsafe:
        let after = block.offset(size)
        if *header(after) & USED == 0:
            unlink(after)
            size += size_of(after)
        if *header(block) & BEFORE_USED == 0:
            let before = *header(block.offset(-2))
            block = block.offset(-before)
            unlink(block)
            size += before
    insert(block, size)

# Whether `payload` now holds `bytes`, without moving: it shrinks, or grows
# into the free block after it.
pub fn resize(payload: *near mut u8, bytes: u16) -> bool:
    let block = payload.offset(-2)
    let size = rounded(bytes)
    let mut whole = size_of(block)
    if whole < size:
        unsafe:
            let after = block.offset(whole)
            if *header(after) & USED != 0 || whole + size_of(after) < size:
                return false
            unlink(after)
            whole += size_of(after)
            *header(block) = whole | (*header(block) & FLAGS)
            *header(block.offset(whole)) |= BEFORE_USED
    trim(block, size)
    return true
