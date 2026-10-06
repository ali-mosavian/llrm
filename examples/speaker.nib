# A tune on the PC speaker. Inline assembly programs the timer's channel 2
# (ports 43h and 42h) and gates it to the speaker (port 61h); the BIOS tick
# count, 18.2 a second, times each note. A key stops the tune early.

const TIMER_HZ: u32 = 1193182

struct Note:
    hz: u16
    ticks: u16

# The BIOS tick count, cx:dx.
fn now() -> u32:
    unsafe:
        asm(ah=0, out=(cx=let high, dx=let low), clobbers=[al, flags]):
            int 1Ah
        return (u32(high) << 16) | u32(low)

fn tone(hz: u16) -> void:
    let divisor = u16(TIMER_HZ // u32(hz))
    unsafe:
        asm(bx=divisor, clobbers=[ax, flags]):
            mov al, 0B6h        ; channel 2, low byte then high, square wave
            out 43h, al
            mov al, bl
            out 42h, al
            mov al, bh
            out 42h, al
            in al, 61h
            or al, 3            ; timer gate and speaker data on
            out 61h, al

fn quiet() -> void:
    unsafe:
        asm(clobbers=[ax, flags]):
            in al, 61h
            and al, 0FCh
            out 61h, al

# Whether a key is waiting; it is taken if so.
fn key_pressed() -> bool:
    unsafe:
        asm(out=(ax=let waiting), clobbers=[flags]):
            mov ah, 1
            int 16h
            mov ax, 0
            jz done
            int 16h             ; ah is 0 after 'mov ax, 0': read the key
            mov ax, 1
            done:
        return waiting != 0

# Waits `ticks` changes of the tick count, so midnight's reset is one more.
fn wait(ticks: u16) -> bool:
    let mut last = now()
    let mut left = ticks
    while left > 0:
        if key_pressed():
            return false
        let current = now()
        if current != last:
            last = current
            left -= 1
    return true

fn main() -> i16:
    let tune: Note[15] = [
        Note(hz=330, ticks=4), Note(hz=330, ticks=4), Note(hz=349, ticks=4), Note(hz=392, ticks=4),
        Note(hz=392, ticks=4), Note(hz=349, ticks=4), Note(hz=330, ticks=4), Note(hz=294, ticks=4),
        Note(hz=262, ticks=4), Note(hz=262, ticks=4), Note(hz=294, ticks=4), Note(hz=330, ticks=4),
        Note(hz=330, ticks=6), Note(hz=294, ticks=2), Note(hz=294, ticks=8),
    ]
    print("playing; a key stops")
    for note in tune:
        tone(note.hz)
        let played = wait(note.ticks)
        quiet()
        if !played:
            print("stopped")
            return 1
        wait(1)
    print("done")
    return 0
