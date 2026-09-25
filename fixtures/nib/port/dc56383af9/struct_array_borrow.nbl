struct point:
    mut x: i16
    y: i16
fn nudge(point: &mut point) -> void:
    point.x += point.y
fn update(points: &mut [point]) -> void:
    for point in &mut points:
        nudge(&mut point)
fn calculate() -> i16:
    let mut points: point[2] = [point(1, 2), point(10, 20)]
    update(&mut points)
    return points[0].x + points[1].x
