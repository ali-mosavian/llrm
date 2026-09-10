"""Non-wrapping symbolic near-address ranges recognized while raising."""

from dataclasses import dataclass

from qbopt.analysis.ranges import Interval
from qbopt.objectfile.module import Addr, Space


@dataclass(frozen=True)
class Region:
    anchor: Addr
    offset: Interval

    def shifted(self, delta: Interval):
        if delta.width != 2 or self.offset.width != 2:
            return None
        low, high = self.offset.low + delta.low, self.offset.high + delta.high
        if not 0 <= self.anchor.disp + low <= self.anchor.disp + high < 65536:
            return None
        return Region(self.anchor, Interval(low, high, 2))

    def overlaps(self, width: int, other: "Addr | Region", size: int):
        if isinstance(other, Addr):
            other = Region(other, Interval(0, 0, 2))
        if self.anchor.space is not other.anchor.space or self.anchor.index != other.anchor.index:
            return False
        return (self.anchor.disp + self.offset.low < other.anchor.disp + other.offset.high + size
                and other.anchor.disp + other.offset.low < self.anchor.disp + self.offset.high + width)


def region(anchor: Addr, offset: Interval, width: int):
    if anchor.space is not Space.SEGMENT or offset.width != 2 or width <= 0:
        return None
    if not 0 <= anchor.disp + offset.low <= anchor.disp + offset.high <= 65536 - width:
        return None
    return Region(anchor, offset)
