# A sensor feed as a pipeline of generators that escape: each stage is a
# value, kept in a variable and handed to the next. The reader holds its
# source open while it yields, and closes it when the consumer stops.

struct Source:
    name: string

fn Source.drop(self: &mut Source) -> void:
    print(f"closed {self.name}")

# The words of `text`, each an owned string.
fn words(text: &string) -> iter[string]:
    let mut word: string = ""
    for c in text:
        if c == ' ':
            if word.len > 0:
                yield word
                word = ""
        else:
            word.push(c)
    if word.len > 0:
        yield word

# The words of `text`, read from a source open while they are.
fn read(name: string, text: &string) -> iter[string]:
    with source = Source(name=name):
        let mut tokens = words(text)
        for word in tokens:
            yield word

fn number(word: &string) -> Option[i16]:
    let mut value: i16 = 0
    for c in word:
        if c < '0' || c > '9':
            return .none
        value = value * 10 + i16(c) - i16('0')
    return .some(value)

# The words that are numbers, as numbers.
fn numbers(words: iter[string]) -> iter[i16]:
    for word in words:
        match number(word):
            .some(value):
                yield value
            .none:
                print(f"skipped {word}")

# The mean of each run of three, from a window kept between yields.
fn smoothed(values: iter[i16]) -> iter[i16]:
    let mut window: i16[3] = [0, 0, 0]
    let mut seen: u16 = 0
    for value in values:
        window[seen % 3] = value
        seen += 1
        if seen >= 3:
            yield (window[0] + window[1] + window[2]) // 3

fn main() -> i16:
    let feed: string = "12 15 x 11 30 28 err 27 9 10"
    let mut means = smoothed(numbers(read("feed", feed)))
    for mean in means:
        print(f"mean {mean}")
    let mut alarm = smoothed(numbers(read("alarm", feed)))
    for mean in alarm:
        if mean > 20:
            print(f"alarm at {mean}")
            break
    print("done")
    return 0
