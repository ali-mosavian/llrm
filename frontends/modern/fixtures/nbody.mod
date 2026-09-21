type scalar = fixed i32, fraction=9

struct vec2i:
    x: scalar
    y: scalar

struct body:
    pos: vec2i
    vel: vec2i

fn nbody(step_count: i32) -> scalar:
    var bodies: [body; 6] = [
        body { pos: vec2i { x: -15, y: -12 }, vel: vec2i { x: 0, y: 0 } },
        body { pos: vec2i { x: -8, y: -7 }, vel: vec2i { x: 0, y: 0 } },
        body { pos: vec2i { x: -1, y: -2 }, vel: vec2i { x: 0, y: 0 } },
        body { pos: vec2i { x: 6, y: 3 }, vel: vec2i { x: 0, y: 0 } },
        body { pos: vec2i { x: 13, y: 8 }, vel: vec2i { x: 0, y: 0 } },
        body { pos: vec2i { x: 20, y: 13 }, vel: vec2i { x: 0, y: 0 } },
    ]
    for step_no in 0..step_count:
        for current in &mut bodies:
            var acc_x: scalar = 0
            var acc_y: scalar = 0
            for other in &bodies:
                if current is not other:
                    let delta_x: scalar = other.pos.x - current.pos.x
                    let delta_y: scalar = other.pos.y - current.pos.y
                    let dist_2: scalar = delta_x * delta_x + delta_y * delta_y + 1
                    let falloff: scalar = 1 / dist_2
                    acc_x = acc_x + delta_x * falloff
                    acc_y = acc_y + delta_y * falloff
            current.vel.x = current.vel.x + acc_x
            current.vel.y = current.vel.y + acc_y
            current.vel.x = current.vel.x - current.vel.x / 16
            current.vel.y = current.vel.y - current.vel.y / 16
        for current in &mut bodies:
            current.pos.x = current.pos.x + current.vel.x
            current.pos.y = current.pos.y + current.vel.y
    for current in &bodies:
        print(f"PX={current.pos.x}")
        print(f"PY={current.pos.y}")
        print(f"VX={current.vel.x}")
        print(f"VY={current.vel.y}")
    print("DONE")
    return bodies[0].pos.x + bodies[1].pos.y + bodies[2].pos.x + bodies[3].pos.y
