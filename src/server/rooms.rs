use thiserror::Error;
use std::{collections::{HashMap, HashSet}, path::Path};
use constcat::concat;

use super::SoaprunServer;

use crate::soaprun::position::Position;
use crate::soaprun::map_attributes::MapAttributes;
use crate::soaprun::rooms::*;

pub const ROOM_EXTENSION : &str = "room";
pub const ROOM_COORD_SEPARATOR : &str = ",";
pub const ROOM_GLOB_EXPRESSION : &str = concat!("*", ROOM_COORD_SEPARATOR, "*.", ROOM_EXTENSION);
pub const DEFAULT_ROOM_NAME : &str = concat!("default.", ROOM_EXTENSION);

#[derive(Error, Debug)]
pub enum LoadRoomError {
    #[error("Failed to parse the filename: `{0}`")]
    NameFormatError(String),
    #[error("Encountered an IO error while reading a room: `{0}`")]
    NewRoomError(#[from] std::io::Error)
}

pub fn load_rooms(path: &Path) -> Result<HashMap<RoomCoordinates, Room>,LoadRoomError>
{
    let buff = path.join(ROOM_GLOB_EXPRESSION);

    let mut rooms = HashMap::new();
    
    for room_path in glob::glob(buff.to_str().unwrap()).unwrap().filter_map(Result::ok)
    {
        let fname = room_path.file_stem().unwrap().to_str().unwrap();
        let coords = match fname.split_once(ROOM_COORD_SEPARATOR) {
            Some(split) => {
                let x = split.0.parse::<i8>();
                let y = split.1.parse::<i8>();
                if x.is_err() || y.is_err() {
                    return Err(LoadRoomError::NameFormatError(fname.to_owned()))
                }
                RoomCoordinates { x: x.unwrap(), y: y.unwrap() }
            },
            None => return Err(LoadRoomError::NameFormatError(fname.to_owned()))
        };
        
        let room = Room::new(&room_path)?;

        rooms.insert(coords, room);
    }
    Ok(rooms)
}


#[derive(serde::Deserialize, Debug)]
pub enum RoomVerificationBounds {
    //Don't check if the room edges/corners make sense
    None,
    //Only check that in-bounds/defined rooms' edges/corners make sense
    InBounds,
    //Check that all room edges/corners make sense, even those bordering on the default/out of bounds room
    All
}
#[derive(serde::Deserialize, Debug)]
pub enum RoomVerificationModes {
    Tiles,
    TileTypes
}
pub fn get_room<'a>(
    rooms: &'a HashMap<RoomCoordinates, Room>, other_coords: &RoomCoordinates, default_room: &'a Room,
    bounds: &RoomVerificationBounds) -> Option<&'a Room>
{
    if matches!(bounds, RoomVerificationBounds::None) ||
    (matches!(bounds, RoomVerificationBounds::InBounds) && !rooms.contains_key(other_coords))
    {
        return None;
    }
    else {
        Some(rooms.get(other_coords).unwrap_or(default_room))
    }
}
fn compare_corners(c1: u8, c2: u8, attributes: Option<&MapAttributes>) -> bool {
    match attributes {
        Some(att) => att.attributes[c1 as usize] == att.attributes[c2 as usize],
        None => c1 == c2
    }
}
fn compare_edges<'a, I1, I2>(it1: I1, it2: I2, attributes: Option<&MapAttributes>) -> bool
where
    I1 : Iterator<Item = &'a u8>,
    I2 : Iterator<Item = &'a u8>
{
    match attributes {
        Some(att) => it1.map(|t| { att.attributes[*t as usize] })
                                 .eq(it2.map(|t| { att.attributes[*t as usize] })),
        None => it1.eq(it2)
    }
}

fn make_room_verification_error(room1_pos: &RoomCoordinates, room2_pos: RoomCoordinates, room1_item: &str, room2_item: &str) -> String {
    format!("Room bounds failed: Room {room1_pos}'s {room1_item} doesn't match with room {room2_pos}'s {room2_item}")
}

//TODO use flood fill instead of this naive algorithm???
//TODO maybe use a macro since real function seem to not like the parameters
pub fn verify_rooms(rooms: &HashMap<RoomCoordinates, Room>, default_room: &Room,
    bounds: &RoomVerificationBounds, attributes: Option<&MapAttributes>) -> Result<(),String>
{
    if !matches!(bounds, RoomVerificationBounds::None) {
        for (pos, room) in rooms {
            let nw = RoomCoordinates {
                x: pos.x.saturating_sub(1),
                y: pos.y.saturating_sub(1)
            };
            if let Some(other) = get_room(rooms, &nw, default_room, bounds) {
                if !compare_corners(room.north_west_corner(), other.south_east_corner(), attributes) {
                    return Err(make_room_verification_error(pos, nw, "north west corner", "south east corner"));
                }
            }

            let n = RoomCoordinates {
                x: pos.x,
                y: pos.y.saturating_sub(1)
            };
            if let Some(other) = get_room(rooms, &n, default_room, bounds) {
                if !compare_edges(room.north_edge(), other.south_edge(), attributes) {
                    return Err(make_room_verification_error(pos, n, "north edge", "south edge"));
                }
            }

            let ne = RoomCoordinates {
                x: pos.x.saturating_add(1),
                y: pos.y.saturating_sub(1)
            };
            if let Some(other) = get_room(rooms, &ne, default_room, bounds) {
                if !compare_corners(room.north_east_corner(), other.south_west_corner(), attributes) {
                    return Err(make_room_verification_error(pos, ne, "north east corner", "south west corner"));
                }
            }

            let w = RoomCoordinates {
                x: pos.x.saturating_sub(1),
                y: pos.y
            };
            if let Some(other) = get_room(rooms, &w, default_room, bounds) {
                if !compare_edges(room.west_edge(), other.east_edge(), attributes) {
                    return Err(make_room_verification_error(pos, w, "west edge", "east edge"));
                }
            }

            let e = RoomCoordinates {
                x: pos.x.saturating_add(1),
                y: pos.y
            };
            if let Some(other) = get_room(rooms, &e, default_room, bounds) {
                if !compare_edges(room.east_edge(), other.west_edge(), attributes) {
                    return Err(make_room_verification_error(pos, e, "east edge", "west edge"));
                }
            }

            let sw = RoomCoordinates {
                x: pos.x.saturating_sub(1),
                y: pos.y.saturating_add(1)
            };
            if let Some(other) = get_room(rooms, &sw, default_room, bounds) {
                if !compare_corners(room.south_west_corner(), other.north_west_corner(), attributes) {
                    return Err(make_room_verification_error(pos, sw, "south west corner", "north east corner"));
                }
            }

            let s = RoomCoordinates {
                x: pos.x,
                y: pos.y.saturating_add(1)
            };
            if let Some(other) = get_room(rooms, &s, default_room, bounds) {
                if !compare_edges(room.south_edge(), other.north_edge(), attributes) {
                    return Err(make_room_verification_error(pos, s, "south edge", "north edge"));
                }
            }

            let se = RoomCoordinates {
                x: pos.x.saturating_add(1),
                y: pos.y.saturating_add(1)
            };
            if let Some(other) = get_room(rooms, &se, default_room, bounds) {
                if !compare_corners(room.south_east_corner(), other.north_west_corner(), attributes) {
                    return Err(make_room_verification_error(pos, se, "north east corner", "south west corner"));
                }
            }
        }
    }
    Ok(())
}

impl SoaprunServer {
    pub fn get_tile(&self, pos: &Position, room: &RoomCoordinates) -> Result<u8,()> {
        let index = pos.to_index(room)?;
        println!("Getting room {room} for reading a tile");
        Ok(match self.rooms.get(room) {
            Some(r) => r.read().unwrap().data[index],
            None => self.default_room.data[index],
        })
    }
    pub fn get_tile_type(&self, pos: &Position, room: &RoomCoordinates) -> Result<u8,()> {
        let tile = self.get_tile(pos, room)?;
        Ok(self.map_attributes.attributes[tile as usize])
    }
    pub fn get_tile_types(&self, pos: Position) -> Vec<u8> {
        Vec::from_iter(self.get_tiles(pos).iter().map(|t| { self.map_attributes.attributes[*t as usize] }))
    }
    pub fn get_tiles(&self, pos: Position) -> Vec<u8> {
        let mut tiles = Vec::with_capacity(4);
        for r in self.get_affected_inbounds_rooms(&pos) {
            let index = pos.to_index(&r).unwrap(); //using to_index on an affected room is always safe
            let room = self.rooms[&r].read().unwrap();
            tiles.push(room.data[index]);
        }
        tiles
    }
    pub fn try_update_tile<UF>(&self, pos: &Position, valid_tiles: &HashSet<u8>, f: UF) -> usize
        where UF : Fn(u8) -> u8
    {
        let mut count = 0;
        for r in self.get_affected_inbounds_rooms(&pos) {
            let index = pos.to_index(&r).unwrap();
            let mut room = self.rooms[&r].write().unwrap();
            
            if valid_tiles.contains(&room.data[index]) {
                let new_val = f(room.data[index]);
                room.data[index] = new_val;
                for (_, p) in self.players.read().unwrap().iter() {
                    let mut pw = p.write().unwrap();
                    if !pw.cached_tiles.contains_key(&r) {
                        pw.cached_tiles.insert(r, HashMap::new());
                    }
                    pw.cached_tiles.get_mut(&r).unwrap().insert(*pos, new_val);
                }
                count += 1;
            }
        }
        count
    }
    pub fn get_affected_inbounds_rooms(&self, pos: &Position) -> HashSet<RoomCoordinates> {
        let mut rooms = HashSet::with_capacity(4);
        rooms.extend(pos.get_affected_rooms().iter().filter(|rc| {
            self.rooms.contains_key(rc)
        }));
        return rooms;
    }
}