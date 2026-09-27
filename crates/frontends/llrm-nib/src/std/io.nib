# Files (std.io): DOS handles owned by a File, which closes its own when
# it is dropped. A failed call is an IoError, never a panic.

import std.os as os

pub enum IoError:
    not_found
    denied
    failed(code: u16)

# DOS's modes for open: read, write, or both.
pub const READ: u8 = 0
pub const WRITE: u8 = 1
pub const READ_WRITE: u8 = 2

const BUFFER = 128
const LONGEST_NAME = 80

pub struct File:
    handle: i16
    mut buffer: u8[BUFFER]
    mut start: u16
    mut end: u16

# DOS's error `code`, as the program sees it.
fn error(code: i16) -> IoError:
    let number = u16(-code)
    if number == 2 || number == 3:
        return .not_found
    if number == 5:
        return .denied
    return .failed(number)

# `path` into `name`, with the NUL DOS reads it up to.
fn named(path: &string, name: &mut char[LONGEST_NAME]) -> void:
    let mut at: u16 = 0
    for letter in path:
        if at < LONGEST_NAME - 1:
            name[at] = letter
            at += 1
    name[at] = '\0'

fn opened(handle: i16) -> Result[File, IoError]:
    if handle < 0:
        return .err(error(handle))
    return .ok(File(handle=handle, buffer=[0] * BUFFER, start=0, end=0))

pub fn File.open(path: &string, mode: u8 = READ) -> Result[File, IoError]:
    let mut name: char[LONGEST_NAME] = ['\0'] * LONGEST_NAME
    named(path, &mut name)
    unsafe:
        return opened(os.open(&name, mode))

# A new, empty file, replacing any of its name.
pub fn File.create(path: &string) -> Result[File, IoError]:
    let mut name: char[LONGEST_NAME] = ['\0'] * LONGEST_NAME
    named(path, &mut name)
    unsafe:
        return opened(os.create(&name))

# `count` bytes from `data`, as they are.
pub fn File.write_raw(self: &mut File, data: *far u8, count: u16) -> Result[u16, IoError]:
    unsafe:
        let written = os.write_file(self.handle, data, count)
        if written < 0:
            return .err(error(written))
        return .ok(u16(written))

pub fn File.write(self: &mut File, text: &string) -> Result[u16, IoError]:
    unsafe:
        let data: *far char = &text
        return self.write_raw(data.cast[u8](), text.len)

# Up to `count` bytes into `data`, those read_line buffered first; fewer
# only at the end of the file.
pub fn File.read_raw(self: &mut File, data: *far mut u8, count: u16) -> Result[u16, IoError]:
    let mut got: u16 = 0
    unsafe:
        while got < count && self.start < self.end:
            data[got] = self.buffer[self.start]
            self.start += 1
            got += 1
        if got < count:
            let read = os.read(self.handle, data.offset(got), count - got)
            if read < 0:
                return .err(error(read))
            got += u16(read)
    return .ok(got)

# The next line without its end, or none at the end of the file.
pub fn File.read_line(self: &mut File) -> Result[Option[string], IoError]:
    let mut line: string = ""
    let mut any = false
    loop:
        if self.start == self.end:
            unsafe:
                let data: *far mut u8 = &mut self.buffer
                let got = os.read(self.handle, data, BUFFER)
                if got < 0:
                    return .err(error(got))
                if got == 0:
                    return .ok(any ? .some(line) : .none)
                self.start = 0
                self.end = u16(got)
        let byte = self.buffer[self.start]
        self.start += 1
        any = true
        if byte == 10:
            return .ok(.some(line))
        if byte != 13:
            line.push(char(byte))

# Each line in turn, until the end or a failure.
pub fn File.lines(self: &mut File) -> iter[string]:
    loop:
        match self.read_line():
            .ok(.some(line)):
                yield line
            _:
                return

pub fn File.drop(self: &mut File) -> void:
    unsafe:
        os.close(self.handle)
