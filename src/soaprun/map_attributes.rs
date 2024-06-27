use std::collections::HashSet;
use std::path::Path;
use std::fs::File;
use std::io::{Read, Error, ErrorKind};

use lazy_static::lazy_static;

//TODO consider making these configurable?
lazy_static! {
    pub static ref CANVAS_TILES : HashSet<u8> = HashSet::from([12,13,14,15,27,28,29,30,31]);
    pub static ref DRAW_TILES : HashSet<u8> = HashSet::from([12,13,14,15]);
    pub static ref MAKE_CORPSE_TILES : HashSet<u8> = HashSet::from([2,4,5,11,12,13,14,15]);
    pub static ref REMOVE_CORPSE_TILES : HashSet<u8> = HashSet::from([18,20,21,27,28,29,30,31]);
}

pub struct MapAttributes {
    pub width: u16,
    pub height: u16,
    pub attributes: Vec<u8>
}
impl MapAttributes {
    pub fn new<P>(path: P) -> Result<MapAttributes,Error> where P : AsRef<Path>
    {
        let mut f = File::open(path)?;

        let mut width_bytes = [0u8; 2];
        f.read_exact(&mut width_bytes)?;
        let width = u16::from_le_bytes(width_bytes);

        let mut height_bytes = [0u8; 2];
        f.read_exact(&mut height_bytes)?;
        let height = u16::from_le_bytes(height_bytes);

        let expected = (width as usize)*(height as usize);
        let mut attributes = Vec::with_capacity(expected);
        f.read_to_end(&mut attributes)?;

        if attributes.len() != expected {
            return Err(Error::from(ErrorKind::InvalidData))
        }
        Ok(MapAttributes { width, height, attributes})
    }
}