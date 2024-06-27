use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
#[derive(Serialize, Deserialize)]
pub struct Position {
    pub x: i16,
    pub y: i16
}
impl Position {
    pub fn in_line(&self, other: &Position) -> bool {
        self.x == other.x || self.y == other.y
    }
    pub fn adjacent_inclusive(&self, other: &Position) -> bool {
        (self.x == other.x && (self.y.wrapping_sub(1) <= other.y && other.y <= self.y.wrapping_add(1))) ||
        (self.y == other.y && (self.x.wrapping_sub(1) <= other.x && other.x <= self.x.wrapping_add(1)))
    }
    pub fn adjacent_exclusive(&self, other: &Position) -> bool {
        (self.x == other.x && (self.y.wrapping_sub(1) == other.y || other.y == self.y.wrapping_add(1))) ||
        (self.y == other.y && (self.x.wrapping_sub(1) == other.x || other.x == self.x.wrapping_add(1)))
    }
    pub fn north(&self, amount: i16) -> Position {
        Position {
            x: self.x,
            y: self.y.saturating_sub(amount)
        }
    }
    pub fn south(&self, amount: i16) -> Position {
        Position {
            x: self.x,
            y: self.y.saturating_add(amount)
        }
    }
    pub fn west(&self, amount: i16) -> Position {
        Position {
            x: self.x.saturating_sub(amount),
            y: self.y
        }
    }
    pub fn east(&self, amount: i16) -> Position {
        Position {
            x: self.x.saturating_add(amount),
            y: self.y
        }
    }
    pub fn taxicab_distance(&self, other: &Position) -> usize {
        self.x.abs_diff(other.x) as usize + self.y.abs_diff(other.y) as usize
    }
}
impl std::fmt::Display for Position
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        return write!(f, "({},{})", self.x, self.y);
    }
}

#[cfg(test)]
mod tests {
    use super::Position;

    #[test]
    fn taxicab_works() {
        let p1 = Position {
            x: 0,
            y: 0
        };
        let p2 = Position {
            x: 0,
            y: 0
        };
        assert!(p1.taxicab_distance(&p2) == 0);

        let p3 = Position {
            x: 1,
            y: 0
        };
        assert!(p1.taxicab_distance(&p3) == 1);

        let p4 = Position {
            x: 1,
            y: 1
        };
        assert!(p1.taxicab_distance(&p4) == 2);

        let p5 = Position {
            x: -1,
            y: -1
        };
        assert!(p1.taxicab_distance(&p5) == 2)
    }
}