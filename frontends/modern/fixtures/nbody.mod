struct vec2i:
    x: i32
    y: i32

struct body:
    pos: vec2i
    vel: vec2i

fn nbody(step_count: i32) -> i32:
    var bodies: [body; 6] = [
        body { pos: vec2i { x: -7680, y: -6144 }, vel: vec2i { x: 0, y: 0 } },
        body { pos: vec2i { x: -4096, y: -3584 }, vel: vec2i { x: 0, y: 0 } },
        body { pos: vec2i { x: -512, y: -1024 }, vel: vec2i { x: 0, y: 0 } },
        body { pos: vec2i { x: 3072, y: 1536 }, vel: vec2i { x: 0, y: 0 } },
        body { pos: vec2i { x: 6656, y: 4096 }, vel: vec2i { x: 0, y: 0 } },
        body { pos: vec2i { x: 10240, y: 6656 }, vel: vec2i { x: 0, y: 0 } },
    ]
    var step_no: i32 = 0
    while step_no < step_count:
        for current in &mut bodies:
            var acc_x: i32 = 0
            var acc_y: i32 = 0
            for other in &bodies:
                if current is not other:
                    let delta_x: i32 = other.pos.x - current.pos.x
                    let delta_y: i32 = other.pos.y - current.pos.y
                    let dist_2: i32 = delta_x * delta_x + delta_y * delta_y + 262144
                    let falloff: i32 = 512 / (dist_2 / 262144 + 1)
                    acc_x = acc_x + (delta_x * falloff) / 512
                    acc_y = acc_y + (delta_y * falloff) / 512
            current.vel.x = current.vel.x + acc_x
            current.vel.y = current.vel.y + acc_y
            current.vel.x = current.vel.x - current.vel.x / 16
            current.vel.y = current.vel.y - current.vel.y / 16
        for current in &mut bodies:
            current.pos.x = current.pos.x + current.vel.x
            current.pos.y = current.pos.y + current.vel.y
        step_no = step_no + 1
    for current in &bodies:
        print(f"PX={current.pos.x}")
        print(f"PY={current.pos.y}")
        print(f"VX={current.vel.x}")
        print(f"VY={current.vel.y}")
    print("DONE")
    return bodies[0].pos.x + bodies[1].pos.y + bodies[2].pos.x + bodies[3].pos.y
