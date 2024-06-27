use bitflags::bitflags;

use crate::soaprun::position::Position;

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct DirectionFlags : u8 {
        const North         = 0b0001;
        const South       = 0b0010;
        const Vertical   = 0b0011; 
        const West       = 0b0100;
        const East      = 0b1000;
        const Horizontal = 0b1100;
    }
}
impl DirectionFlags {
    pub fn to_positions(&self, pos: &Position) -> Vec<Position> {
        let mut positions = Vec::with_capacity(4);
        for f in self.iter() {
            match f {
                DirectionFlags::North    => positions.push(pos.north(1)),
                DirectionFlags::South  => positions.push(pos.south(1)),
                DirectionFlags::West  => positions.push(pos.west(1)),
                DirectionFlags::East => positions.push(pos.east(1)),
                _ => unreachable!()
            }
        }
        positions
    }
}

impl Position {
    pub fn relative_direction(&self, other: &Position) -> DirectionFlags {
        let mut flags = DirectionFlags::empty();
        if other.x < self.x {
            flags.insert(DirectionFlags::West);
        } else if self.x < other.x {
            flags.insert(DirectionFlags::East)
        }

        if other.y < self.y {
            flags.insert(DirectionFlags::North);
        } else if self.y < other.y {
            flags.insert(DirectionFlags::South)
        }

        flags
    }
}

#[cfg(test)]
mod tests {
    use super::DirectionFlags;
    use crate::soaprun::position::Position;

    #[test]
    fn relative_direction_works() {
        let p = Position {
            x: 0,
            y: 0
        };
        let e = Position {
            x: 1,
            y: 0
        };
        let w = Position {
            x: -2,
            y: 0
        };
        let n = Position {
            x: 0,
            y: -200
        };
        let s = Position {
            x: 0,
            y: 232
        };

        assert!(matches!(p.relative_direction(&e), DirectionFlags::East));
        assert!(matches!(p.relative_direction(&w), DirectionFlags::West));
        assert!(matches!(p.relative_direction(&n), DirectionFlags::North));
        assert!(matches!(p.relative_direction(&s), DirectionFlags::South));

        assert!(w.relative_direction(&n) == DirectionFlags::North | DirectionFlags::East);
        assert!(e.relative_direction(&n) == DirectionFlags::North | DirectionFlags::West);
        assert!(w.relative_direction(&s) == DirectionFlags::South | DirectionFlags::East);
        assert!(e.relative_direction(&s) == DirectionFlags::South | DirectionFlags::West);

        assert!(n.relative_direction(&w) == DirectionFlags::South | DirectionFlags::West);
        assert!(s.relative_direction(&w) == DirectionFlags::North | DirectionFlags::West);
        assert!(n.relative_direction(&e) == DirectionFlags::South | DirectionFlags::East);
        assert!(s.relative_direction(&e) == DirectionFlags::North | DirectionFlags::East);
    }
}