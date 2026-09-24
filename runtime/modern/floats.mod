# Floats as text (section 13): the shortest decimal that reads back as the
# same f32 or f64, in Python's repr form. The digits come from Burger and
# Dybvig's free-format algorithm, exact in big integers kept on the heap.

import format
import heap

# A big integer is LIMBS u16 limbs, least significant first: enough for an
# f64's smallest subnormal scaled by 10^324.
const LIMBS = 72
const BYTES: u16 = 144
const ZERO: u8 = 48

fn assign(a: *near mut u16, value: u16) -> void:
    unsafe:
        for at in range(0, LIMBS):
            a[at] = 0
        a[0] = value

fn copy(to: *near mut u16, source: *near mut u16) -> void:
    unsafe:
        for at in range(0, LIMBS):
            to[at] = source[at]

# `a` times 2^places.
fn shift(a: *near mut u16, places: u16) -> void:
    let words = places >> 4
    let rest = places & 15
    unsafe:
        let mut at: u16 = LIMBS
        while at != 0:
            at -= 1
            a[at] = at >= words ? a[at - words] : 0
        if rest != 0:
            at = LIMBS
            while at > 1:
                at -= 1
                a[at] = (a[at] << rest) | (a[at - 1] >> (16 - rest))
            a[0] = a[0] << rest

fn multiply(a: *near mut u16, factor: u16) -> void:
    let mut carry: u32 = 0
    unsafe:
        for at in range(0, LIMBS):
            let product = u32(a[at]) * factor + carry
            a[at] = u16(product)
            carry = product >> 16

fn add(a: *near mut u16, b: *near mut u16) -> void:
    let mut carry: u32 = 0
    unsafe:
        for at in range(0, LIMBS):
            let sum = u32(a[at]) + b[at] + carry
            a[at] = u16(sum)
            carry = sum >> 16

# `a` less `b`, which is no larger.
fn subtract(a: *near mut u16, b: *near mut u16) -> void:
    let mut borrow: u32 = 0
    unsafe:
        for at in range(0, LIMBS):
            let difference = u32(a[at]) - b[at] - borrow
            a[at] = u16(difference)
            borrow = (difference >> 16) & 1

fn compare(a: *near mut u16, b: *near mut u16) -> i8:
    let mut at: u16 = LIMBS
    unsafe:
        while at != 0:
            at -= 1
            if a[at] != b[at]:
                return a[at] < b[at] ? -1 : 1
    return 0

# Whether `a` is past `limit`, or reaches it when `inclusive`.
fn beyond(a: *near mut u16, limit: *near mut u16, inclusive: bool) -> bool:
    let order = compare(a, limit)
    return order > 0 || (inclusive && order == 0)

fn put(text: *near mut u8, at: u16, byte: u8) -> u16:
    unsafe:
        text[at] = byte
    return at + 1

fn put_text(text: *near mut u8, at: u16, word: &string) -> u16:
    let mut end = at
    for letter in word:
        end = put(text, end, u8(letter))
    return end

# The value `f * 2^e` as digits into `out`, and the power of ten k with
# value = 0.digits * 10^k. `boundary` when f is the smallest mantissa of
# its binade, whose gap below is half the gap above; `even` when f is even,
# so a decimal on a gap's edge still reads back as the value.
fn digits(numbers: *near mut u16, e: i16, boundary: bool, even: bool, out: *near mut u8) -> (u16, i16):
    let r = numbers
    let s = numbers.offset(LIMBS)
    let above = numbers.offset(2 * LIMBS)
    let below = numbers.offset(3 * LIMBS)
    let t = numbers.offset(4 * LIMBS)
    let wide: u16 = boundary ? 1 : 0
    # r / s is the value, and above / s and below / s the gaps to its
    # neighbours, halved.
    if e >= 0:
        shift(r, u16(e) + 1 + wide)
        assign(s, 2 << wide)
        assign(below, 1)
        shift(below, u16(e))
    else:
        shift(r, 1 + wide)
        assign(s, 1)
        shift(s, u16(-e) + 1 + wide)
        assign(below, 1)
    copy(above, below)
    shift(above, wide)
    # The smallest k with r + above within s * 10^k.
    let mut k: i16 = 0
    loop:
        copy(t, r)
        add(t, above)
        if !beyond(t, s, !even):
            break
        multiply(s, 10)
        k += 1
    loop:
        copy(t, r)
        add(t, above)
        multiply(t, 10)
        if beyond(t, s, !even):
            break
        multiply(r, 10)
        multiply(above, 10)
        multiply(below, 10)
        k -= 1
    let mut count: u16 = 0
    loop:
        multiply(r, 10)
        multiply(above, 10)
        multiply(below, 10)
        let mut digit: u8 = 0
        while compare(r, s) >= 0:
            subtract(r, s)
            digit += 1
        let low = !beyond(r, below, !even)
        copy(t, r)
        add(t, above)
        let high = beyond(t, s, even)
        if !low && !high:
            count = put(out, count, ZERO + digit)
            continue
        if low && high:
            copy(t, r)
            shift(t, 1)
            if compare(t, s) >= 0:
                digit += 1
        else if high:
            digit += 1
        count = put(out, count, ZERO + digit)
        return (count, k)

# Python's repr of the float in `words`, least significant first: `places`
# bits of its fraction in the top word under an `exponent` field of that
# mask, biased by `bias` counted in whole-mantissa units.
fn print_float(words: *far u16, count: u16, places: u16, exponent: u16, bias: i16) -> void:
    let block = heap.allocate(5 * BYTES + 32)
    let numbers = block.cast[u16]()
    let text = block.offset(5 * BYTES)
    let mut at: u16 = 0
    unsafe:
        let top = words[count - 1]
        let biased = (top >> places) & exponent
        let high_fraction = top & ((u16(1) << places) - 1)
        let mut empty = high_fraction == 0
        for word in range(0, count - 1):
            empty = empty && words[word] == 0
        if top >> 15 != 0 && !(biased == exponent && !empty):
            at = put(text, at, 45)
        if biased == exponent:
            at = put_text(text, at, empty ? "inf" : "nan")
        else if biased == 0 && empty:
            at = put_text(text, at, "0.0")
        else:
            assign(numbers, 0)
            for word in range(0, count - 1):
                numbers[word] = words[word]
            numbers[count - 1] = high_fraction | (biased != 0 ? u16(1) << places : 0)
            let e = (biased != 0 ? i16(biased) : 1) - bias
            let digit_text = text.offset(16)
            let (length, k) = digits(numbers, e, biased > 1 && empty, words[0] & 1 == 0, digit_text)
            at = decimal(text, at, digit_text, length, k - 1)
    format.number(text, at)
    heap.release(block)

# `length` digits whose first is worth 10^point, as repr writes them.
fn decimal(text: *near mut u8, start: u16, digits: *near mut u8, length: u16, point: i16) -> u16:
    let mut at = start
    unsafe:
        if point >= -4 && point < 16:
            if point < 0:
                at = put_text(text, at, "0.")
                for _zero in range(0, -point - 1):
                    at = put(text, at, ZERO)
                for index in range(0, length):
                    at = put(text, at, digits[index])
                return at
            let whole = u16(point) + 1
            for index in range(0, whole):
                at = put(text, at, index < length ? digits[index] : ZERO)
            at = put(text, at, 46)
            if length <= whole:
                return put(text, at, ZERO)
            for index in range(whole, length):
                at = put(text, at, digits[index])
            return at
        at = put(text, at, digits[0])
        if length > 1:
            at = put(text, at, 46)
            for index in range(1, length):
                at = put(text, at, digits[index])
        at = put(text, at, 101)
        at = put(text, at, point < 0 ? 45 : 43)
        let magnitude = u16(point < 0 ? -point : point)
        if magnitude >= 100:
            at = put(text, at, ZERO + u8(magnitude // 100))
        at = put(text, at, ZERO + u8(magnitude // 10 % 10))
        return put(text, at, ZERO + u8(magnitude % 10))

export "cdecl16":
    @link_name("M$PR8")
    fn print_r8(value: f64) -> void:
        let mut stored = value
        unsafe:
            let data: *far f64 = &stored
            print_float(data.cast[u16](), 4, 4, 0x7FF, 1075)

    @link_name("M$PR4")
    fn print_r4(value: f32) -> void:
        let mut stored = value
        unsafe:
            let data: *far f32 = &stored
            print_float(data.cast[u16](), 2, 7, 0xFF, 150)
