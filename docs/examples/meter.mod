# A game's health and score: health saturates at its bounds, and a score
# that would overflow is reported rather than wrapped (section 3).

struct Player:
    mut health: u8
    mut score: i32

fn Player.hit(self: &mut Player, damage: u8) -> void:
    self.health = self.health.saturating_sub(damage)

fn Player.heal(self: &mut Player, amount: u8) -> void:
    self.health = self.health.saturating_add(amount)

fn Player.award(self: &mut Player, points: i32, multiplier: i32) -> bool:
    match points.checked_mul(multiplier):
        .some(bonus):
            match self.score.checked_add(bonus):
                .some(total):
                    self.score = total
                    return true
                .none:
                    return false
        .none:
            return false

fn main() -> i16:
    let mut player = Player(health=100, score=2000000000)
    player.hit(30)
    player.heal(200)
    print(f"health {player.health}")
    player.hit(255)
    print(f"health {player.health}")
    print(f"{player.award(1000, 100000)} {player.score}")
    print(f"{player.award(100000, 100000)} {player.score}")
    print(f"{player.award(100000000, 2)} {player.score}")
    return 0
