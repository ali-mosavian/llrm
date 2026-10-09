//! Nib's runtime heap and string routines, modelled on the
//! host (crates/frontends/llrm-nib/src/runtime/buffers.nib and strings.nib). A dropped buffer is marked, so a
//! second drop or a leak is an execution error, not silent.

use super::model::{DescriptorField, DescriptorPlace};
use super::*;
use crate::abi::nib as rt;

const HEAP: u8 = 0x01;
const READONLY: u8 = 0x08;
const FREED: u8 = 0x80;

/// The word of `width` bytes at `address` and `delta`.
fn word(
    address: &Address,
    delta: i64,
    width: usize,
) -> Outcome<i128> {
    let cells = address.memory.borrow();
    let at = usize::try_from(address.offset + delta).ok().filter(|one| one + width <= cells.bytes.len());
    let Some(at) = at else {
        return fail("descriptor read outside its buffer");
    };
    let mut bytes = [0; 16];
    bytes[..width].copy_from_slice(&cells.bytes[at..at + width]);
    Ok(i128::from_le_bytes(bytes))
}

/// A heap buffer's header: where the field `field` is, relative to its data.
fn at(
    field: DescriptorField,
    width: usize,
) -> i64 {
    DescriptorPlace::heap_offset(field, width as i64)
}

fn header(width: usize) -> i64 {
    DescriptorPlace::header_bytes(width as i64)
}

fn flags(
    address: &Address,
    width: usize,
) -> Outcome<u8> {
    let cells = address.memory.borrow();
    match usize::try_from(address.offset - header(width)).ok().and_then(|at| cells.bytes.get(at)) {
        Some(flags) if flags & FREED != 0 => fail("use of a dropped buffer"),
        Some(flags) => Ok(*flags),
        None => fail("a string has no descriptor"),
    }
}

/// A string's bytes, by its descriptor's length.
fn text(
    address: &Address,
    width: usize,
) -> Outcome<Vec<u8>> {
    flags(address, width)?;
    let length = word(address, at(DescriptorField::Length, width), width)? as usize;
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

fn set_word(
    address: &Address,
    field: DescriptorField,
    width: usize,
    value: usize,
) {
    let start = (address.offset + at(field, width)) as usize;
    address.memory.borrow_mut().bytes[start..start + width].copy_from_slice(&value.to_le_bytes()[..width]);
}

fn set_length(
    address: &Address,
    width: usize,
    length: usize,
) {
    set_word(address, DescriptorField::Length, width, length);
}

/// `count` bytes of `from`'s data into `to`'s, with the addresses among them.
fn copy_data(
    from: &Address,
    to: &Address,
    count: usize,
) {
    let (start, at) = (from.offset, to.offset);
    let source = from.memory.borrow();
    let mut target = to.memory.borrow_mut();
    let (start_byte, at_byte) = (start as usize, at as usize);
    target.bytes[at_byte..at_byte + count].copy_from_slice(&source.bytes[start_byte..start_byte + count]);
    for ((offset, width), address) in &source.pointers {
        if (start..start + count as i64).contains(offset) {
            target.pointers.insert((offset - start + at, *width), address.clone());
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
    fn allocate(
        &mut self,
        bytes: &[u8],
        length: usize,
        capacity: usize,
        size: usize,
    ) -> Address {
        let width = self.word;
        // The flags word: the flags byte, then its pad.
        let mut cells = vec![HEAP];
        cells.resize(width, 0);
        cells.extend(&length.to_le_bytes()[..width]);
        cells.extend(&capacity.to_le_bytes()[..width]);
        cells.extend(bytes);
        cells.resize(header(width) as usize + capacity.max(length) * size + 1, 0);
        let memory = Rc::new(RefCell::new(Cells { bytes: cells, pointers: HashMap::default(), dead: false }));
        self.heap.push(memory.clone());
        Address { memory, offset: header(width), length: None, capacity: None }
    }

    /// `address` on the heap, writable, with room for `wanted` elements.
    fn reserve(
        &mut self,
        address: &Address,
        wanted: usize,
        size: usize,
    ) -> Outcome<Address> {
        let width = self.word;
        flags(address, width)?;
        let length = word(address, at(DescriptorField::Length, width), width)? as usize;
        let room = word(address, at(DescriptorField::Capacity, width), width)? as usize;
        let wanted = wanted.max(length);
        if flags(address, width)? & (HEAP | READONLY) == HEAP && room >= wanted {
            return Ok(address.clone());
        }
        let copy = self.allocate(&[], 0, wanted.max(2 * room), size);
        copy_data(address, &copy, length * size + 1);
        set_length(&copy, width, length);
        self.drop_buffer(address)?;
        Ok(copy)
    }

    /// crates/frontends/llrm-nib/src/runtime/dicts.nib's `N$DRES`: room for one more entry.
    fn dict_reserve(
        &mut self,
        table: &Address,
        size: usize,
    ) -> Outcome<Address> {
        let width = self.word;
        flags(table, width)?;
        let slots = word(table, at(DescriptorField::Length, width), width)? as usize;
        let count = word(table, at(DescriptorField::Capacity, width), width)? as usize;
        if (count + 1) * 4 <= slots * 3 {
            return Ok(table.clone());
        }
        let grown = if slots < 8 { 8 } else { slots * 2 };
        let copy = self.allocate(&[], 0, grown, size);
        for slot in 0..slots {
            let entry = Address { offset: table.offset + (slot * size) as i64, ..table.clone() };
            let hash = word(&entry, 0, width)? as usize;
            if hash == 0 {
                continue;
            }
            let mut at = hash & (grown - 1);
            while word(&Address { offset: copy.offset + (at * size) as i64, ..copy.clone() }, 0, width)? != 0 {
                at = (at + 1) & (grown - 1);
            }
            copy_data(&entry, &Address { offset: copy.offset + (at * size) as i64, ..copy.clone() }, size);
        }
        set_length(&copy, width, grown);
        set_word(&copy, DescriptorField::Capacity, width, count);
        self.drop_buffer(table)?;
        Ok(copy)
    }

    fn drop_buffer(
        &mut self,
        address: &Address,
    ) -> Outcome<()> {
        if flags(address, self.word)? & HEAP != 0 {
            address.memory.borrow_mut().bytes[(address.offset - header(self.word)) as usize] |= FREED;
        }
        Ok(())
    }

    fn append(
        &mut self,
        to: &Address,
        more: &[u8],
    ) -> Outcome<Address> {
        let width = self.word;
        let mut bytes = text(to, width)?;
        let heap = flags(to, width)? & (HEAP | READONLY) == HEAP;
        let capacity = word(to, at(DescriptorField::Capacity, width), width)? as usize;
        bytes.extend(more);
        if heap && capacity >= bytes.len() {
            let mut cells = to.memory.borrow_mut();
            let start = to.offset as usize;
            cells.bytes[start..start + bytes.len()].copy_from_slice(&bytes);
            cells.bytes[start + bytes.len()] = 0;
            drop(cells);
            set_length(to, width, bytes.len());
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
            rt::BUFFER_DROP => {
                if let Some(address) = pointer(&arguments[0])? {
                    self.drop_buffer(&address)?;
                }
                None
            }
            rt::BUFFER_RESERVE | rt::BUFFER_GROW | rt::BUFFER_SHRINK | rt::BUFFER_CLONE => {
                let Some(address) = pointer(&arguments[0])? else {
                    return fail(format!("{name} of null"));
                };
                let width = self.word;
                let length = word(&address, at(DescriptorField::Length, width), width)? as usize;
                match name {
                    rt::BUFFER_RESERVE => {
                        Some(Scalar::Address(self.reserve(&address, size(&arguments[1])?, size(&arguments[2])?)?))
                    }
                    rt::BUFFER_GROW => {
                        let count = size(&arguments[1])?;
                        let grown = self.reserve(&address, length + count, size(&arguments[2])?)?;
                        set_length(&grown, width, length + count);
                        Some(Scalar::Address(grown))
                    }
                    rt::BUFFER_SHRINK => {
                        let Some(rest) = length.checked_sub(size(&arguments[1])?) else {
                            return fail("pop from an empty vec");
                        };
                        set_length(&address, width, rest);
                        Some(Scalar::Int(rest as i128))
                    }
                    _ => {
                        let size = size(&arguments[1])?;
                        let copy = self.allocate(&[], 0, length, size);
                        copy_data(&address, &copy, length * size + 1);
                        set_length(&copy, width, length);
                        Some(Scalar::Address(copy))
                    }
                }
            }
            rt::TEXT_CONCAT | rt::TEXT_APPEND => {
                let (Some(left), Some(right)) = (pointer(&arguments[0])?, pointer(&arguments[1])?) else {
                    return fail(format!("{name} of null"));
                };
                let more = text(&right, self.word)?;
                let target = if name == rt::TEXT_CONCAT {
                    let bytes = text(&left, self.word)?;
                    self.allocate(&bytes, bytes.len(), bytes.len() + more.len(), 1)
                } else {
                    left
                };
                Some(Scalar::Address(self.append(&target, &more)?))
            }
            rt::DICT_RESERVE => {
                let Some(table) = pointer(&arguments[0])? else {
                    return fail(format!("{} of null", rt::DICT_RESERVE));
                };
                Some(Scalar::Address(self.dict_reserve(&table, size(&arguments[1])?)?))
            }
            rt::ERROR_KEY => return self.panic("key not found"),
            rt::ERROR_BOUNDS => return self.panic("index out of bounds"),
            rt::ERROR_SHIFT => return self.panic("shift count out of range"),
            rt::ERROR_CONVERT => return self.panic("float outside the integer type"),
            rt::VIEW_COMPARE => {
                let (left, right) =
                    (descriptor_bytes(&arguments[0], self.word)?, descriptor_bytes(&arguments[1], self.word)?);
                Some(Scalar::Int(left.cmp(&right) as i128))
            }
            rt::VIEW_COPY => {
                let bytes = descriptor_bytes(&arguments[0], self.word)?;
                Some(Scalar::Address(self.allocate(&bytes, bytes.len(), bytes.len(), 1)))
            }
            rt::PRINT_BEGIN => {
                let sink = self.allocate(&[], 0, 16, 1);
                self.sink = Some(sink);
                None
            }
            rt::PRINT_END => match self.sink.take() {
                Some(sink) => Some(Scalar::Address(sink)),
                None => return fail(format!("{} without {}", rt::PRINT_END, rt::PRINT_BEGIN)),
            },
            symbol if rt::os_operation(symbol).is_some() => Some(Scalar::Int(i128::from(self.file(name, arguments)?))),
            _ => return Ok(None),
        };
        Ok(Some(result))
    }

    /// The DOS file calls on host files: a handle or count, or DOS's error
    /// code negated, as the OS layer's operations return them (runtime/shared/interface.toml).
    fn file(
        &mut self,
        name: &str,
        arguments: &[Scalar],
    ) -> Outcome<i16> {
        use std::io::{Read, Write};
        const FIRST: usize = 5;
        let failed = |error: std::io::Error| -> i16 {
            match error.kind() {
                std::io::ErrorKind::NotFound => -rt::error_code("not_found"),
                std::io::ErrorKind::PermissionDenied => -rt::error_code("denied"),
                _ => -31,
            }
        };
        let operation = rt::os_operation(name);
        if matches!(operation, Some("open" | "create")) {
            let Some(path) = pointer(&arguments[0])? else {
                return fail(format!("{name} of null"));
            };
            let bytes = path.memory.borrow().bytes[path.offset as usize..].to_vec();
            let path: String = bytes.iter().take_while(|one| **one != 0).map(|one| *one as char).collect();
            let mut options = std::fs::OpenOptions::new();
            match operation == Some("create") {
                true => options.write(true).create(true).truncate(true),
                false => match size(&arguments[1])? {
                    0 => options.read(true),
                    1 => options.write(true),
                    _ => options.read(true).write(true),
                },
            };
            return Ok(match options.open(&path) {
                Ok(file) => {
                    self.files.push(Some(file));
                    (FIRST + self.files.len() - 1) as i16
                }
                Err(error) => failed(error),
            });
        }
        match operation {
            Some("console_read_key") => return Ok(self.input.pop_front().map_or(0, i16::from)),
            Some("console_key_ready") => return Ok(i16::from(!self.input.is_empty())),
            _ => {}
        }
        let handle = size(&arguments[0])?;
        if operation == Some("write_file") && handle == rt::standard_handle("stdout") {
            let bytes = view_bytes(&arguments[1], &arguments[2])?;
            self.emit(&cp437(&bytes))?;
            return Ok(bytes.len() as i16);
        }
        if operation == Some("read") && handle == rt::standard_handle("stdin") {
            let Some(data) = pointer(&arguments[1])? else {
                return fail(format!("{name} of null"));
            };
            let count = size(&arguments[2])?.min(self.input.len());
            let bytes: Vec<u8> = self.input.drain(..count).collect();
            let start = data.offset as usize;
            let mut cells = data.memory.borrow_mut();
            let Some(target) = cells.bytes.get_mut(start..start + count) else {
                return fail(format!("{name} outside its buffer"));
            };
            target.copy_from_slice(&bytes);
            return Ok(count as i16);
        }
        let Some(Some(file)) = handle.checked_sub(FIRST).and_then(|at| self.files.get_mut(at)) else {
            return Ok(-6);
        };
        Ok(match operation {
            Some("read") => {
                let Some(data) = pointer(&arguments[1])? else {
                    return fail(format!("{name} of null"));
                };
                let mut bytes = vec![0; size(&arguments[2])?];
                match file.read(&mut bytes) {
                    Ok(count) => {
                        let start = data.offset as usize;
                        let mut cells = data.memory.borrow_mut();
                        let Some(target) = cells.bytes.get_mut(start..start + count) else {
                            return fail(format!("{name} outside its buffer"));
                        };
                        target.copy_from_slice(&bytes[..count]);
                        count as i16
                    }
                    Err(error) => failed(error),
                }
            }
            Some("write_file") => {
                let bytes = view_bytes(&arguments[1], &arguments[2])?;
                match file.write_all(&bytes) {
                    Ok(()) => bytes.len() as i16,
                    Err(error) => failed(error),
                }
            }
            _ => {
                self.files[handle - FIRST] = None;
                0
            }
        })
    }

    /// Formatted text goes to the console, or while an f-string builds, into it.
    pub(super) fn emit(
        &mut self,
        text: &str,
    ) -> Outcome<()> {
        match self.sink.take() {
            Some(sink) => {
                let bytes: Vec<u8> = text.chars().map(|one| one as u8).collect();
                self.sink = Some(self.append(&sink, &bytes)?);
            }
            None => self.output.push_str(text),
        }
        Ok(())
    }

    /// Ends the program the way the DOS runtime's panics do.
    pub(super) fn panic<T>(
        &mut self,
        message: &str,
    ) -> Outcome<T> {
        self.panicked = Some(message.to_owned());
        fail(format!("panic: {message}"))
    }

    /// Heap buffers never dropped.
    pub(super) fn leaked(&self) -> usize {
        self.heap.iter().filter(|one| one.borrow().bytes[0] & FREED == 0).count()
    }
}

/// A view's bytes, by its data pointer and length.
pub(super) fn view_bytes(
    data: &Scalar,
    length: &Scalar,
) -> Outcome<Vec<u8>> {
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

/// A `&string` view's bytes, by the far pointer to its descriptor: the
/// length at 0 and the far data pointer after its length and capacity words.
pub(super) fn descriptor_bytes(
    descriptor: &Scalar,
    width: usize,
) -> Outcome<Vec<u8>> {
    let Some(at) = pointer(descriptor)? else {
        return fail("a view of null");
    };
    let (length, data) = {
        let cells = at.memory.borrow();
        let start = at.offset as usize;
        let length = cells
            .bytes
            .get(start..start + width)
            .map(
                |word| {
                    let mut bytes = [0; 8];
                    bytes[..width].copy_from_slice(word);
                    u64::from_le_bytes(bytes)
                },
            );
        (length, cells.pointers.get(&(at.offset + 2 * width as i64, 4)).cloned())
    };
    match (length, data) {
        (Some(length), Some(data)) => view_bytes(&Scalar::Address(data), &Scalar::Int(i128::from(length))),
        _ => fail("a view descriptor outside its storage"),
    }
}

/// `N$PS`'s bytes: a string's, by its descriptor.
pub(super) fn string_bytes(
    argument: &Scalar,
    width: usize,
) -> Outcome<Vec<u8>> {
    match pointer(argument)? {
        Some(address) => text(&address, width),
        None => fail(format!("{} of null", rt::PRINT_STRING)),
    }
}
