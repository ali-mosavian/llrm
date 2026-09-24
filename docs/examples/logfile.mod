# A log that closes itself: `Log.drop` runs when its owner ends, on every
# path, and a moved Log is closed once, by its new owner. The std.io File
# inside it closes its DOS handle after.

import std.io as io

struct Log:
    name: string
    mut file: io.File
    mut lines: u16

fn Log.create(name: string) -> Result[Log, io.IoError]:
    let file = io.File.create(&name)?
    return .ok(Log(name=name, file=file, lines=0))

fn Log.write(self: &mut Log, text: &string) -> void:
    let _ = self.file.write(text)
    let _ = self.file.write("\r\n")
    self.lines += 1

fn Log.drop(self: &mut Log) -> void:
    print(f"closed {self.name} after {self.lines} lines")

fn keep_open(log: Log) -> Log:
    return log

fn main() -> Result[void, io.IoError]:
    let mut log = Log.create("LOG.TXT")?
    log.write("started")
    with mut scratch = Log.create("SCRATCH.TXT")?:
        scratch.write("temporary")
    let mut kept = keep_open(log)
    kept.write("moved, still open")
    print("done")
    return .ok()
