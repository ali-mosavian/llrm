# A club roster: players print through `display`, are compared by identity
# with `is`, are built by a function called on their type, and are ranked
# with a comprehension of tuples.

enum Fault:
    bad_rating(rating: i16)

struct Player:
    name: string
    mut rating: i16

fn Player.new(name: &string, rating: i16) -> Result[Player, Fault]:
    if rating < 0:
        return .err(.bad_rating(rating))
    return .ok(Player(name=name.copy(), rating=rating))

fn Player.display(self: &Player) -> string:
    return f"{self.name} ({self.rating})"

fn Player.grade(self: &Player) -> string:
    if self.rating >= 2000:
        return "master"
    else if self.rating >= 1500:
        return "expert"
    else:
        return "club"

fn best(players: &vec[Player]) -> &Player:
    let mut top = 0
    for i in range(0, players.len):
        if players[i].rating > players[top].rating:
            top = i
    return players[top]

fn sign_up(players: &mut vec[Player], name: &string, rating: i16) -> Result[void, Fault]:
    players.push(Player.new(name, rating)?)
    return .ok()

fn main() -> i16:
    const bonus: i16 = 25
    let mut players: vec[Player] = []
    for (name, rating) in [("ada", 2150), ("bob", 1480), ("cy", 1620)]:
        let _ = sign_up(players, name, rating)
    match sign_up(players, "dee", -3):
        .ok():
            print("signed dee")
        .err(.bad_rating(r)):
            print(f"refused rating {r}")
    players[1].rating += bonus
    let top = best(players)
    for player in players:
        let mark = player is top ? " *" : ""
        print(f"{player} {player.grade()}{mark}")
    let strong = [(players[i].rating, i) for i in range(0, players.len) if players[i].rating > 1500]
    let (rating, at) = strong[0]
    print(f"{strong.len} above 1500, first {players[at].name} at {rating}")
    return 0
