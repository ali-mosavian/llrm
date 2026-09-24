# A file that closes itself: `File.drop` runs when its owner ends, on every
# path, and a moved File is closed once, by its new owner.

extern "cdecl16":
    fn dos_create(name: *far char) -> i16
    fn dos_write(handle: i16, data: *far char, count: u16) -> i16
    fn dos_close(handle: i16) -> void

struct File:
    name: string
    handle: i16
    lines: u16

fn File.create(name: string) -> File:
    let mut handle: i16 = -1
    unsafe:
        handle = dos_create(&name)
    return File(name=name, handle=handle, lines=0)

fn File.write(self: &mut File, text: &string) -> void:
    let end = "\r\n"
    unsafe:
        dos_write(self.handle, &text, text.len)
        dos_write(self.handle, &end, end.len)
    self.lines += 1

fn File.drop(self: &mut File) -> void:
    unsafe:
        dos_close(self.handle)
    print(f"closed {self.name} after {self.lines} lines")

fn keep_open(file: File) -> File:
    return file

fn main() -> i16:
    let mut log = File.create("LOG.TXT")
    if log.handle < 0:
        print("cannot create LOG.TXT")
        return 1
    log.write("started")
    with mut scratch = File.create("SCRATCH.TXT"):
        scratch.write("temporary")
    let mut kept = keep_open(log)
    kept.write("moved, still open")
    print("done")
    return 0
