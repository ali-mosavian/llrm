struct point:
    mut x: i16
    y: i16
fn calculate() -> i16:
    let mut current: point = point(x=1, y=2)
    let snapshot = current
    current = point(x=current.y, y=current.x)
    current.x += snapshot.x
    return current.x * 10 + current.y
