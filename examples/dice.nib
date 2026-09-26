# A seeded dice: `for` calls Dice.iter() for an iterator, then Rolls.next()
# until it returns .none (section 12).

struct Dice:
    seed: u16
    rolls: u16

struct Rolls:
    mut state: u16
    mut left: u16

fn Dice.iter(self: &Dice) -> Rolls:
    return Rolls(state=self.seed, left=self.rolls)

fn Rolls.next(self: &mut Rolls) -> Option[u16]:
    if self.left == 0:
        return .none
    self.left -= 1
    self.state = self.state * 25173 + 13849
    return .some((self.state >> 8) % 6 + 1)

fn main() -> i16:
    let dice = Dice(seed=2024, rolls=60)
    let mut counts: u16[6] = [0] * 6
    for face in dice:
        counts[face - 1] += 1
    for face in 0..6:
        print(f"{face + 1}: {counts[face]:2}")
    for face in Rolls(state=7, left=100):
        if face == 6:
            print("first six")
            break
    return 0
