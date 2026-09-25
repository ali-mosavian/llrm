struct sample:
    tag: i16
    mut value: i32
    delta: i32

fn update() -> i32:
    let mut samples: sample[5] = [
        sample(tag=0, value=1, delta=2),
        sample(tag=0, value=2, delta=3),
        sample(tag=0, value=3, delta=4),
        sample(tag=0, value=4, delta=5),
        sample(tag=0, value=5, delta=6),
    ]
    for current in &mut samples:
        current.value += current.delta
    return samples[0].value + samples[4].value

fn main() -> i16:
    update()
    return 0
