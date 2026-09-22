struct point:
    x: i32
    y: i32
struct body:
    pos: point
    vel: point
fn calculate() -> i32:
    let bodies: [body; 2] = [
        { pos: { x: 1, y: 2 }, vel: { x: 3, y: 4 } },
        {{5, 6}, {7, 8}},
    ]
    return bodies[0].pos.x + bodies[0].pos.y * 10 + bodies[0].vel.x * 100 + bodies[0].vel.y * 1000 + bodies[1].pos.x * 10000 + bodies[1].pos.y * 100000 + bodies[1].vel.x * 1000000 + bodies[1].vel.y * 10000000
