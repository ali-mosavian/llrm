type scalar = fixed i32, fraction=9

struct vec2i:
    mut x: scalar
    mut y: scalar

struct body:
    mut pos: vec2i
    mut vel: vec2i

fn nbody(step_count: i32) -> scalar:
    let mut bodies: body[6] = [
        body(vec2i(-15, -12), vec2i(0, 0)),
        body(vec2i(-8, -7), vec2i(0, 0)),
        body(vec2i(-1, -2), vec2i(0, 0)),
        body(vec2i(6, 3), vec2i(0, 0)),
        body(vec2i(13, 8), vec2i(0, 0)),
        body(vec2i(20, 13), vec2i(0, 0)),
    ]
    for step_no in 0..step_count:
        for current in &mut bodies:
            let mut acc = vec2i(x=0, y=0)
            for other in &bodies:
                if current is not other:
                    let delta = vec2i(
                        x=other.pos.x - current.pos.x,
                        y=other.pos.y - current.pos.y,
                    )
                    let dist_2: scalar = delta.x * delta.x + delta.y * delta.y + 1
                    let falloff: scalar = 1 / dist_2
                    acc.x += delta.x * falloff
                    acc.y += delta.y * falloff
            current.vel.x += acc.x
            current.vel.y += acc.y
            current.vel.x -= current.vel.x / 16
            current.vel.y -= current.vel.y / 16
        for current in &mut bodies:
            current.pos.x += current.vel.x
            current.pos.y += current.vel.y
    for current in &bodies:
        print(f"PX={current.pos.x}")
        print(f"PY={current.pos.y}")
        print(f"VX={current.vel.x}")
        print(f"VY={current.vel.y}")
    print("DONE")
    return bodies[0].pos.x + bodies[1].pos.y + bodies[2].pos.x + bodies[3].pos.y

fn main() -> i16:
    nbody(1)
    return 0
