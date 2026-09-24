# Implicitly available in every module (draft section 14).

enum Option[T]:
    some(T)
    none

enum Result[T, E]:
    ok(T)
    err(E)

# The library generators (draft section 12): a `for` compiles each in place.

fn enumerate[T](items: &[T]) -> iter[(u16, &T)]:
    let mut i: u16 = 0
    for item in items:
        yield (i, item)
        i += 1

fn zip[A, B](left: &[A], right: &[B]) -> iter[(&A, &B)]:
    let mut i: u16 = 0
    while i < left.len && i < right.len:
        yield (&left[i], &right[i])
        i += 1

fn range[T](start: T, end: T) -> iter[T]:
    let mut i = start
    while i < end:
        yield i
        i += 1
