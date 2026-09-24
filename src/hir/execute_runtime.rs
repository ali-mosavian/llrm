//! The modern-language runtime's heap and string routines, modelled on the
//! host (runtime/modern/heap.c and text.c). A dropped buffer is marked, so a
//! second drop or a leak is an execution error, not silent.

use super::*;

const HEAP: u8 = 0x01;
const READONLY: u8 = 0x08;
const FREED: u8 = 0x80;

fn word(address: &Address, delta: i64) -> Outcome<i128> {
    let cells = address.memory.borrow();
    let at = usize::try_from(address.offset + delta)
        .ok()
        .filter(|one| one + 2 <= cells.bytes.len());
    let Some(at) = at else {
        return fail("descriptor read outside its buffer");
    };
    Ok(i128::from(u16::from_le_bytes([
        cells.bytes[at],
        cells.bytes[at + 1],
    ])))
}

fn flags(address: &Address) -> Outcome<u8> {
    let cells = address.memory.borrow();
    match usize::try_from(address.offset - 6)
        .ok()
        .and_then(|at| cells.bytes.get(at))
    {
        Some(flags) if flags & FREED != 0 => fail("use of a dropped buffer"),
        Some(flags) => Ok(*flags),
        None => fail("a string has no descriptor"),
    }
}

/// A string's bytes, by its descriptor's length.
fn text(address: &Address) -> Outcome<Vec<u8>> {
    flags(address)?;
    let length = word(address, -4)? as usize;
    let cells = address.memory.borrow();
    let start = address.offset as usize;
    cells
        .bytes
        .get(start..start + length)
        .map(<[u8]>::to_vec)
        .ok_or_else(|| ExecutionError("string length outside its buffer".into()))
}

fn size(argument: &Scalar) -> Outcome<usize> {
    Ok(argument.whole()? as usize)
}

fn set_length(address: &Address, length: usize) {
    let start = address.offset as usize;
    address.memory.borrow_mut().bytes[start - 4..start - 2]
        .copy_from_slice(&(length as u16).to_le_bytes());
}

/// `count` bytes of `from`'s data into `to`'s, with the addresses among them.
fn copy_data(from: &Address, to: &Address, count: usize) {
    let (start, at) = (from.offset, to.offset);
    let source = from.memory.borrow();
    let mut target = to.memory.borrow_mut();
    let (start_byte, at_byte) = (start as usize, at as usize);
    target.bytes[at_byte..at_byte + count]
        .copy_from_slice(&source.bytes[start_byte..start_byte + count]);
    for ((offset, width), address) in &source.pointers {
        if (start..start + count as i64).contains(offset) {
            target
                .pointers
                .insert((offset - start + at, *width), address.clone());
        }
    }
}

fn pointer(argument: &Scalar) -> Outcome<Option<Address>> {
    match argument {
        Scalar::Address(address) => Ok(Some(address.clone())),
        Scalar::Int(0) => Ok(None),
        _ => fail("expected a string pointer"),
    }
}

impl Machine<'_> {
    /// A new heap buffer holding `bytes`, with room for `capacity` elements of `size` bytes.
    fn allocate(&mut self, bytes: &[u8], length: usize, capacity: usize, size: usize) -> Address {
        let mut cells = vec![HEAP, 0];
        cells.extend((length as u16).to_le_bytes());
        cells.extend((capacity as u16).to_le_bytes());
        cells.extend(bytes);
        cells.resize(6 + capacity.max(length) * size + 1, 0);
        let memory = Rc::new(RefCell::new(Cells {
            bytes: cells,
            pointers: HashMap::default(),
                        dead: false,
        }));
        self.heap.push(memory.clone());
        Address {
            memory,
            offset: 6,
            length: None,
            capacity: None,
        }
    }

    /// `address` on the heap, writable, with room for `wanted` elements.
    fn reserve(&mut self, address: &Address, wanted: usize, size: usize) -> Outcome<Address> {
        flags(address)?;
        let length = word(address, -4)? as usize;
        let room = word(address, -2)? as usize;
        let wanted = wanted.max(length);
        if flags(address)? & (HEAP | READONLY) == HEAP && room >= wanted {
            return Ok(address.clone());
        }
        let copy = self.allocate(&[], 0, wanted.max(2 * room), size);
        copy_data(address, &copy, length * size + 1);
        set_length(&copy, length);
        self.drop_buffer(address)?;
        Ok(copy)
    }

    /// runtime/modern/dict.c's `rt_dict_reserve`: room for one more entry.
    fn dict_reserve(&mut self, table: &Address, size: usize) -> Outcome<Address> {
        flags(table)?;
        let slots = word(table, -4)? as usize;
        let count = word(table, -2)? as usize;
        if (count + 1) * 4 <= slots * 3 {
            return Ok(table.clone());
        }
        let grown = if slots < 8 { 8 } else { slots * 2 };
        let copy = self.allocate(&[], 0, grown, size);
        for slot in 0..slots {
            let entry = Address { offset: table.offset + (slot * size) as i64, ..table.clone() };
            let hash = word(&entry, 0)? as usize;
            if hash == 0 {
                continue;
            }
            let mut at = hash & (grown - 1);
            while word(&Address { offset: copy.offset + (at * size) as i64, ..copy.clone() }, 0)? != 0 {
                at = (at + 1) & (grown - 1);
            }
            copy_data(&entry, &Address { offset: copy.offset + (at * size) as i64, ..copy.clone() }, size);
        }
        set_length(&copy, grown);
        copy.memory.borrow_mut().bytes[copy.offset as usize - 2..copy.offset as usize].copy_from_slice(&(count as u16).to_le_bytes());
        self.drop_buffer(table)?;
        Ok(copy)
    }

    fn drop_buffer(&mut self, address: &Address) -> Outcome<()> {
        if flags(address)? & HEAP != 0 {
            address.memory.borrow_mut().bytes[address.offset as usize - 6] |= FREED;
        }
        Ok(())
    }

    fn append(&mut self, to: &Address, more: &[u8]) -> Outcome<Address> {
        let mut bytes = text(to)?;
        let heap = flags(to)? & (HEAP | READONLY) == HEAP;
        let capacity = word(to, -2)? as usize;
        bytes.extend(more);
        if heap && capacity >= bytes.len() {
            let mut cells = to.memory.borrow_mut();
            let start = to.offset as usize;
            cells.bytes[start..start + bytes.len()].copy_from_slice(&bytes);
            cells.bytes[start + bytes.len()] = 0;
            cells.bytes[start - 4..start - 2].copy_from_slice(&(bytes.len() as u16).to_le_bytes());
            return Ok(to.clone());
        }
        let grown = self.allocate(&bytes, bytes.len(), bytes.len().max(2 * capacity), 1);
        self.drop_buffer(to)?;
        Ok(grown)
    }

    /// `Some(result)` when `name` is a heap or string routine.
    pub(super) fn runtime(
        &mut self,
        name: &str,
        arguments: &[Scalar],
    ) -> Outcome<Option<Option<Scalar>>> {
        let result = match name {
            "_rt_drop" => {
                if let Some(address) = pointer(&arguments[0])? {
                    self.drop_buffer(&address)?;
                }
                None
            }
            "_rt_reserve" | "_rt_grow" | "_rt_shrink" | "_rt_clone" => {
                let Some(address) = pointer(&arguments[0])? else {
                    return fail(format!("{name} of null"));
                };
                let length = word(&address, -4)? as usize;
                match name {
                    "_rt_reserve" => Some(Scalar::Address(self.reserve(
                        &address,
                        size(&arguments[1])?,
                        size(&arguments[2])?,
                    )?)),
                    "_rt_grow" => {
                        let count = size(&arguments[1])?;
                        let grown = self.reserve(&address, length + count, size(&arguments[2])?)?;
                        set_length(&grown, length + count);
                        Some(Scalar::Address(grown))
                    }
                    "_rt_shrink" => {
                        let Some(rest) = length.checked_sub(size(&arguments[1])?) else {
                            return fail("pop from an empty vec");
                        };
                        set_length(&address, rest);
                        Some(Scalar::Int(rest as i128))
                    }
                    _ => {
                        let size = size(&arguments[1])?;
                        let copy = self.allocate(&[], 0, length, size);
                        copy_data(&address, &copy, length * size + 1);
                        set_length(&copy, length);
                        Some(Scalar::Address(copy))
                    }
                }
            }
            "_rt_concat" | "_rt_append" => {
                let (Some(left), Some(right)) = (pointer(&arguments[0])?, pointer(&arguments[1])?)
                else {
                    return fail(format!("{name} of null"));
                };
                let more = text(&right)?;
                let target = if name == "_rt_concat" {
                    let bytes = text(&left)?;
                    self.allocate(&bytes, bytes.len(), bytes.len() + more.len(), 1)
                } else {
                    left
                };
                Some(Scalar::Address(self.append(&target, &more)?))
            }
            "_rt_compare" => {
                let (Some(left), Some(right)) = (pointer(&arguments[0])?, pointer(&arguments[1])?)
                else {
                    return fail("_rt_compare of null");
                };
                Some(Scalar::Int(text(&left)?.cmp(&text(&right)?) as i128))
            }
            "_rt_dict_reserve" => {
                let Some(table) = pointer(&arguments[0])? else {
                    return fail("_rt_dict_reserve of null");
                };
                Some(Scalar::Address(self.dict_reserve(&table, size(&arguments[1])?)?))
            }
            "_rt_panic_key" => return self.panic("key not found"),
            "_rt_panic_bounds" => return self.panic("index out of bounds"),
            "_rt_panic_shift" => return self.panic("shift count out of range"),
            "_rt_panic_convert" => return self.panic("float outside the integer type"),
            "_rt_view_compare" => {
                let (left, right) = (
                    view_bytes(&arguments[0], &arguments[1])?,
                    view_bytes(&arguments[2], &arguments[3])?,
                );
                Some(Scalar::Int(left.cmp(&right) as i128))
            }
            "_rt_view_copy" => {
                let bytes = view_bytes(&arguments[0], &arguments[1])?;
                Some(Scalar::Address(self.allocate(
                    &bytes,
                    bytes.len(),
                    bytes.len(),
                    1,
                )))
            }
            "_rt_begin" => {
                let sink = self.allocate(&[], 0, 16, 1);
                self.sink = Some(sink);
                None
            }
            "_rt_end" => match self.sink.take() {
                Some(sink) => Some(Scalar::Address(sink)),
                None => return fail("_rt_end without _rt_begin"),
            },
            _ => return Ok(None),
        };
        Ok(Some(result))
    }

    /// Formatted text goes to the console, or while an f-string builds, into it.
    pub(super) fn emit(&mut self, text: &str) -> Outcome<()> {
        match self.sink.take() {
            Some(sink) => {
                let bytes: Vec<u8> = text.chars().map(|one| one as u8).collect();
                self.sink = Some(self.append(&sink, &bytes)?);
            }
            None => self.output.push_str(text),
        }
        Ok(())
    }

    /// Ends the program the way the DOS runtime's `rt_panic` does.
    pub(super) fn panic<T>(&mut self, message: &str) -> Outcome<T> {
        self.panicked = Some(message.to_owned());
        fail(format!("panic: {message}"))
    }

    /// Heap buffers never dropped.
    pub(super) fn leaked(&self) -> usize {
        self.heap
            .iter()
            .filter(|one| one.borrow().bytes[0] & FREED == 0)
            .count()
    }
}

/// A view's bytes, by its data pointer and length.
pub(super) fn view_bytes(data: &Scalar, length: &Scalar) -> Outcome<Vec<u8>> {
    let (Some(address), length) = (pointer(data)?, size(length)?) else {
        return fail("a view of null");
    };
    let cells = address.memory.borrow();
    let start = address.offset as usize;
    cells
        .bytes
        .get(start..start + length)
        .map(<[u8]>::to_vec)
        .ok_or_else(|| ExecutionError("view outside its buffer".into()))
}

/// `_pt`'s bytes: a string's, by its descriptor.
pub(super) fn string_bytes(argument: &Scalar) -> Outcome<Vec<u8>> {
    match pointer(argument)? {
        Some(address) => text(&address),
        None => fail("_pt of null"),
    }
}
