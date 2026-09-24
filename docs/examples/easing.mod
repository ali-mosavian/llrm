# Easing curves for a sprite's slide across the screen: each curve is a
# function value, and the table of them is a vec.

const STEPS = 5
const WIDTH: i16 = 40

fn linear(t: i16) -> i16:
    return t

fn ease_in(t: i16) -> i16:
    return t * t // 100

fn ease_out(t: i16) -> i16:
    return 100 - ease_in(100 - t)

# Where a sprite starts, ends, and how it gets there.
struct Slide:
    start: i16
    end: i16
    curve: fn(i16) -> i16

fn Slide.at(self: &Slide, t: i16) -> i16:
    let f = self.curve
    return self.start + (self.end - self.start) * f(t) // 100

fn main() -> i16:
    fn smooth(t: i16) -> i16:
        # Ease in for the first half, out for the second.
        if t < 50:
            return ease_in(t * 2) // 2
        return 50 + ease_out(t * 2 - 100) // 2
    let names: vec[string] = ["linear", "in", "out", "smooth", "step"]
    let curves: vec[fn(i16) -> i16] = [linear, ease_in, ease_out, smooth, |t| t // 50 * 50]
    for i in 0..curves.len:
        let slide = Slide(start=0, end=WIDTH, curve=curves[i])
        let mut line: string = f"{names[i]:-6} "
        for step in 0..STEPS + 1:
            line.append(f" {slide.at(step * 100 // STEPS):3}")
        print(line)
    return 0
