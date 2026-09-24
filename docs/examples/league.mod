# A small league table: methods on a struct, a generic function, lambdas,
# generators consumed by for loops, and tuples returned and taken apart.

struct Team:
    name: string
    mut won: i16
    mut drawn: i16
    mut lost: i16

fn Team.points(self: &Team) -> i16:
    return self.won * 3 + self.drawn

fn Team.played(self: &Team) -> i16:
    return self.won + self.drawn + self.lost

fn Team.record(self: &mut Team, scored: i16, conceded: i16) -> void:
    if scored > conceded:
        self.won += 1
    else:
        if scored == conceded:
            self.drawn += 1
        else:
            self.lost += 1

# The index and value of the largest of `values`.
fn best[T](values: &[T]) -> (u16, T):
    let mut at: u16 = 0
    let mut top = values[0]
    let mut i: u16 = 0
    for value in values:
        if value > top:
            top = value
            at = i
        i += 1
    return (at, top)

# Each index whose value `keep` accepts.
fn where[T, F](values: &[T], keep: F) -> iter[u16]:
    let mut i: u16 = 0
    for value in values:
        if keep(value):
            yield i
        i += 1

fn main() -> i16:
    let mut teams: vec[Team] = []
    for name in ["Rovers", "United", "Athletic"]:
        teams.push(Team(name=name.copy(), won=0, drawn=0, lost=0))
    teams[0].record(2, 1)
    teams[0].record(0, 0)
    teams[1].record(3, 0)
    teams[1].record(1, 2)
    teams[2].record(1, 1)
    teams[2].record(0, 4)

    let points = [team.points() for team in teams]
    let (leader, top) = best(points)
    print(f"{teams[leader].name} lead on {top}")

    let needed = 2
    for i in where(points, |p: i16| p >= needed):
        let team = teams[i].copy()
        print(f"{team.name}: {team.points()} from {team.played()}")
    return 0
