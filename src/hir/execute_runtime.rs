//! Nib's runtime heap and string routines, modelled on the
//! host (runtime/nib/buffers.nib and strings.nib). A dropped buffer is marked, so a
//! second drop or a leak is an execution error, not silent.

use crate::abi::nib as rt;
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

    /// runtime/nib/dicts.nib's `N$DRES`: room for one more entry.
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
                let length = word(&address, -4)? as usize;
                match name {
                    rt::BUFFER_RESERVE => Some(Scalar::Address(self.reserve(
                        &address,
                        size(&arguments[1])?,
                        size(&arguments[2])?,
                    )?)),
                    rt::BUFFER_GROW => {
                        let count = size(&arguments[1])?;
                        let grown = self.reserve(&address, length + count, size(&arguments[2])?)?;
                        set_length(&grown, length + count);
                        Some(Scalar::Address(grown))
                    }
                    rt::BUFFER_SHRINK => {
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
            rt::TEXT_CONCAT | rt::TEXT_APPEND => {
                let (Some(left), Some(right)) = (pointer(&arguments[0])?, pointer(&arguments[1])?)
                else {
                    return fail(format!("{name} of null"));
                };
                let more = text(&right)?;
                let target = if name == rt::TEXT_CONCAT {
                    let bytes = text(&left)?;
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
                let (left, right) = (descriptor_bytes(&arguments[0])?, descriptor_bytes(&arguments[1])?);
                Some(Scalar::Int(left.cmp(&right) as i128))
            }
            rt::VIEW_COPY => {
                let bytes = descriptor_bytes(&arguments[0])?;
                Some(Scalar::Address(self.allocate(
                    &bytes,
                    bytes.len(),
                    bytes.len(),
                    1,
                )))
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
            rt::FILE_OPEN | rt::FILE_CREATE | rt::FILE_READ | rt::FILE_WRITE | rt::FILE_CLOSE => {
                Some(Scalar::Int(i128::from(self.file(name, arguments)?)))
            }
            _ => return Ok(None),
        };
        Ok(Some(result))
    }

    /// The DOS file calls on host files: a handle or count, or DOS's error
    /// code negated, as runtime/nib/dos.asm returns them.
    fn file(&mut self, name: &str, arguments: &[Scalar]) -> Outcome<i16> {
        use std::io::{Read, Write};
        const FIRST: usize = 5;
        const STANDARD_OUTPUT: usize = 1;
        let failed = |error: std::io::Error| -> i16 {
            match error.kind() {
                std::io::ErrorKind::NotFound => -2,
                std::io::ErrorKind::PermissionDenied => -5,
                _ => -31,
            }
        };
        if name == rt::FILE_OPEN || name == rt::FILE_CREATE {
            let Some(path) = pointer(&arguments[0])? else {
                return fail(format!("{name} of null"));
            };
            let bytes = path.memory.borrow().bytes[path.offset as usize..].to_vec();
            let path: String = bytes.iter().take_while(|one| **one != 0).map(|one| *one as char).collect();
            let mut options = std::fs::OpenOptions::new();
            match name == rt::FILE_CREATE {
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
        let handle = size(&arguments[0])?;
        if name == rt::FILE_WRITE && handle == STANDARD_OUTPUT {
            let bytes = view_bytes(&arguments[1], &arguments[2])?;
            self.emit(&cp437(&bytes))?;
            return Ok(bytes.len() as i16);
        }
        let Some(Some(file)) = handle.checked_sub(FIRST).and_then(|at| self.files.get_mut(at)) else {
            return Ok(-6);
        };
        Ok(match name {
            rt::FILE_READ => {
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
            rt::FILE_WRITE => {
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

    /// Ends the program the way the DOS runtime's panics do.
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

/// A `&string` view's bytes, by the far pointer to its descriptor: the
/// length at 0 and the far data pointer at 4.
pub(super) fn descriptor_bytes(descriptor: &Scalar) -> Outcome<Vec<u8>> {
    let Some(at) = pointer(descriptor)? else {
        return fail("a view of null");
    };
    let (length, data) = {
        let cells = at.memory.borrow();
        let start = at.offset as usize;
        let length = cells.bytes.get(start..start + 2).map(|word| u16::from_le_bytes([word[0], word[1]]));
        (length, cells.pointers.get(&(at.offset + 4, 4)).cloned())
    };
    match (length, data) {
        (Some(length), Some(data)) => view_bytes(&Scalar::Address(data), &Scalar::Int(i128::from(length))),
        _ => fail("a view descriptor outside its storage"),
    }
}

/// `N$PS`'s bytes: a string's, by its descriptor.
pub(super) fn string_bytes(argument: &Scalar) -> Outcome<Vec<u8>> {
    match pointer(argument)? {
        Some(address) => text(&address),
        None => fail(format!("{} of null", rt::PRINT_STRING)),
    }
}
