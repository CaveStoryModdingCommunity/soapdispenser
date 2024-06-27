use std::collections::HashSet;
use std::path::Path;
use std::io::{Error, ErrorKind};

use super::position::Position;

pub const CLIENT_ROOM_WIDTH : usize = 21;
pub const CLIENT_ROOM_HEIGHT : usize = 16;
pub const MIN_X_COORD : i16 = ((CLIENT_ROOM_WIDTH - 1) as i16) * (i8::MIN as i16);
pub const MIN_Y_COORD : i16 = ((CLIENT_ROOM_HEIGHT - 1) as i16) * (i8::MIN as i16);
pub const MAX_X_COORD : i16 = ((CLIENT_ROOM_WIDTH - 1) as i16) * (i8::MAX as i16 + 1);
pub const MAX_Y_COORD : i16 = ((CLIENT_ROOM_HEIGHT - 1) as i16) * (i8::MAX as i16 + 1);


#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
pub struct RoomCoordinates
{
    pub x: i8,
    pub y: i8
}
impl std::fmt::Display for RoomCoordinates
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        return write!(f, "({},{})", self.x, self.y);
    }
}

impl Position {
    pub fn to_index(&self, room: &RoomCoordinates) -> Result<usize,()> {
        
        let x = (self.x as isize - (room.x as isize * (CLIENT_ROOM_WIDTH - 1) as isize)) as isize;
        let y = (self.y as isize - (room.y as isize * (CLIENT_ROOM_HEIGHT - 1) as isize)) as isize;
        if 0 <= x && x < CLIENT_ROOM_WIDTH as isize
        && 0 <= y && y < CLIENT_ROOM_HEIGHT as isize {
            Ok((y as usize * CLIENT_ROOM_WIDTH) + x as usize)
        }
        else {
            Err(())
        }
    }
    pub fn on_horizontal_edge(&self) -> bool {
        (self.x % ((CLIENT_ROOM_WIDTH - 1) as i16)) == 0
    }
    pub fn on_vertical_edge(&self) -> bool {
        (self.y % ((CLIENT_ROOM_HEIGHT - 1) as i16)) == 0
    }
    pub fn on_edge(&self) -> bool {
        self.on_horizontal_edge() || self.on_vertical_edge()
    }
    pub fn get_affected_rooms(&self) -> HashSet<RoomCoordinates> {
        let mut result = HashSet::with_capacity(4);

        //this method of determining the room is biased towards the north west...
        let x = self.x / ((CLIENT_ROOM_WIDTH  - 1) as i16);
        let y = self.y / ((CLIENT_ROOM_HEIGHT - 1) as i16);
        let base = RoomCoordinates {
            x: x.clamp(i8::MIN as i16, i8::MAX as i16) as i8,
            y: y.clamp(i8::MIN as i16, i8::MAX as i16) as i8
        };
        result.insert(base);

        //...so we need an extra check down here for the south east case
        let on_horizontal_edge = self.x < MAX_X_COORD && self.on_horizontal_edge();
        let on_vertical_edge = self.y < MAX_Y_COORD && self.on_vertical_edge();

        if on_horizontal_edge {
            let h = RoomCoordinates {
                x: base.x.saturating_sub(1),
                y: base.y
            };
            result.insert(h);
        }
        if on_vertical_edge {
            let v = RoomCoordinates {
                x: base.x,
                y: base.y.saturating_sub(1)
            };
            result.insert(v);
        }
        if on_horizontal_edge && on_vertical_edge {
            let hv = RoomCoordinates {
                x: base.x.saturating_sub(1),
                y: base.y.saturating_sub(1)
            };
            result.insert(hv);
        }
        result
    }
}

#[derive(Clone, Copy)]
pub struct Room
{
    pub data: [u8; CLIENT_ROOM_WIDTH*CLIENT_ROOM_HEIGHT]
}
impl Room
{
    pub fn new<P>(path: P) -> Result<Room,Error> where P : AsRef<Path>
    {
        match std::fs::read(path)
        {
            Ok(data) =>
                if data.len() == CLIENT_ROOM_WIDTH*CLIENT_ROOM_HEIGHT {
                    Ok(Room { data: data.try_into().unwrap() })
                }
                else {
                    Err(Error::from(ErrorKind::InvalidData))
                },
            Err(e) => Err(e),
        }
    }
    
    pub fn north_west_corner(&self) -> u8
    {
        return self.data[0];
    }
    pub fn north_east_corner(&self) -> u8
    {
        return self.data[CLIENT_ROOM_WIDTH-1];
    }
    pub fn south_west_corner(&self) -> u8
    {
        return self.data[CLIENT_ROOM_WIDTH*(CLIENT_ROOM_HEIGHT-1)];
    }
    pub fn south_east_corner(&self) -> u8
    {
        return self.data[(CLIENT_ROOM_WIDTH*CLIENT_ROOM_HEIGHT)-1];
    }

    pub fn north_edge(&self) -> impl Iterator<Item = &u8>
    {
        return self.data.iter().take(CLIENT_ROOM_WIDTH);
    }
    pub fn west_edge(&self) -> impl Iterator<Item = &u8>
    {
        return self.data.iter().step_by(CLIENT_ROOM_WIDTH);
    }
    pub fn east_edge(&self) -> impl Iterator<Item = &u8>
    {
        return self.data.iter().skip(CLIENT_ROOM_WIDTH-1).step_by(CLIENT_ROOM_WIDTH);
    }
    pub fn south_edge(&self) -> impl Iterator<Item = &u8>
    {
        return self.data.iter().skip(CLIENT_ROOM_WIDTH*(CLIENT_ROOM_HEIGHT-1))
        .take(CLIENT_ROOM_WIDTH*CLIENT_ROOM_HEIGHT);
    }
}

#[cfg(test)]
mod tests {


    use std::collections::HashSet;

    use super::{Room, CLIENT_ROOM_HEIGHT, CLIENT_ROOM_WIDTH};

    #[test]
    fn compare_works() {
        let r1 = Room {
            data: [0; CLIENT_ROOM_WIDTH*CLIENT_ROOM_HEIGHT]
        };
        let r2 = Room {
            data: [0; CLIENT_ROOM_WIDTH*CLIENT_ROOM_HEIGHT]
        };
        assert!(r1.north_west_corner() == r2.south_east_corner());
        assert!(r1.north_east_corner() == r2.south_west_corner());
        assert!(r1.south_west_corner() == r2.north_east_corner());
        assert!(r1.south_east_corner() == r2.north_west_corner());

        assert!(r1.west_edge().eq(r2.east_edge()));
        assert!(r1.north_edge().eq(r2.south_edge()));
        assert!(r1.east_edge().eq(r2.west_edge()));
        assert!(r1.south_edge().eq(r2.north_edge()));
    }

    use crate::soaprun::position::Position;
    use crate::soaprun::rooms::{RoomCoordinates, MAX_X_COORD, MAX_Y_COORD, MIN_X_COORD, MIN_Y_COORD};
    use crate::soaprun::soaprunners::{CLIENT_SPAWN_POSITION, CLIENT_SPAWN_ROOM};

    #[test]
    fn get_affected_rooms_works() {
        assert_eq!(CLIENT_SPAWN_POSITION.get_affected_rooms(), HashSet::from([CLIENT_SPAWN_ROOM]));

        let north_west_pos = Position {
            x: MIN_X_COORD,
            y: MIN_Y_COORD
        };
        let north_west_room = RoomCoordinates {
            x: i8::MIN,
            y: i8::MIN
        };
        assert_eq!(north_west_pos.get_affected_rooms(), HashSet::from([north_west_room]));

        let north_east_pos = Position {
            x: MAX_X_COORD,
            y: MIN_Y_COORD
        };
        let north_east_room = RoomCoordinates {
            x: i8::MAX,
            y: i8::MIN
        };
        assert_eq!(north_east_pos.get_affected_rooms(), HashSet::from([north_east_room]));

        let south_west_pos = Position {
            x: MIN_X_COORD,
            y: MAX_Y_COORD
        };
        let south_west_room = RoomCoordinates {
            x: i8::MIN,
            y: i8::MAX
        };
        assert_eq!(south_west_pos.get_affected_rooms(), HashSet::from([south_west_room]));

        let south_east_pos = Position {
            x: MAX_X_COORD,
            y: MAX_Y_COORD
        };
        let south_east_room = RoomCoordinates {
            x: i8::MAX,
            y: i8::MAX
        };
        assert_eq!(south_east_pos.get_affected_rooms(), HashSet::from([south_east_room]));
    }

    fn test_to_index_room(rc: RoomCoordinates) {
        let x_offset = rc.x as isize * (CLIENT_ROOM_WIDTH - 1) as isize;
        let y_offset = rc.y as isize * (CLIENT_ROOM_HEIGHT - 1) as isize;
        for x in 0..CLIENT_ROOM_WIDTH as isize {
            for y in 0..CLIENT_ROOM_HEIGHT as isize {
                let p = Position {
                    x: (x_offset + x) as i16,
                    y: (y_offset + y) as i16
                };
                let expected = (y as usize * CLIENT_ROOM_WIDTH) + x as usize;
                let actual = p.to_index(&rc).unwrap();
                assert_eq!(expected, actual);
            }
        }
    }

    #[test]
    fn to_index_0_0_works() {
        test_to_index_room(RoomCoordinates {
            x: 0,
            y: 0
        })
    }

    #[test]
    fn to_index_1_1_works() {
        test_to_index_room(RoomCoordinates {
            x: 1,
            y: 1
        })
    }

    #[test]
    fn to_index_neg_1_neg_1_works() {
        test_to_index_room(RoomCoordinates {
            x: -1,
            y: -1
        })
    }

    #[test]
    fn oob_to_index_errors() {
        let rc = RoomCoordinates {
            x: 0,
            y: 0
        };

        for x in -1 .. CLIENT_ROOM_WIDTH as i16 +1 {
            let p = Position {
                x,
                y: -1
            };
            assert!(p.to_index(&rc).is_err());
            
            let p = Position {
                x,
                y: CLIENT_ROOM_HEIGHT as i16
            };
            assert!(p.to_index(&rc).is_err());
        }

        for y in -1 .. CLIENT_ROOM_HEIGHT as i16 +1 {
            let p = Position {
                x: -1,
                y
            };
            assert!(p.to_index(&rc).is_err());
            
            let p = Position {
                x: CLIENT_ROOM_WIDTH as i16,
                y
            };
            assert!(p.to_index(&rc).is_err());
        }

    }


    #[test]
    fn to_index_works() {
        //first crawl in november map
        let r1 = RoomCoordinates {
            x: 1,
            y: 0
        };
        let r2 = RoomCoordinates {
            x: 1,
            y: 1
        };
        let p1 = Position {
            x: 36,
            y: 15
        };
        assert_eq!(p1.to_index(&r1).unwrap(), (CLIENT_ROOM_HEIGHT * CLIENT_ROOM_WIDTH) - 5);
        assert_eq!(p1.to_index(&r2).unwrap(), CLIENT_ROOM_WIDTH - 5);

        let r1 = RoomCoordinates {
            x: -1,
            y: -1
        };
        let p1 = Position {
            x: 0,
            y: 0
        };
        assert_eq!(p1.to_index(&r1).unwrap(), (CLIENT_ROOM_HEIGHT*CLIENT_ROOM_WIDTH) - 1);
        
        let min_room = RoomCoordinates {
            x: i8::MIN,
            y: i8::MIN
        };
        let min_pos = Position {
            x: MIN_X_COORD,
            y: MIN_Y_COORD
        };
        assert_eq!(min_pos.to_index(&min_room).unwrap(), 0);

        let max_room = RoomCoordinates {
            x: i8::MAX,
            y: i8::MAX
        };
        let max_pos = Position {
            x: MAX_X_COORD,
            y: MAX_Y_COORD
        };
        assert_eq!(max_pos.to_index(&max_room).unwrap(), (CLIENT_ROOM_HEIGHT*CLIENT_ROOM_WIDTH) - 1);
    }
}