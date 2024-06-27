use bitflags::bitflags;
use super::rooms::RoomCoordinates;
use super::position::Position;

#[repr(u8)]
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub enum SoaprunnerSprites {
    Idle = 0,
    Walking,
    Dying,
    Winning,
    Ghost
}
#[repr(u8)]
#[derive(Clone, Copy, Debug)]
pub enum SoaprunnerColors {
    Green = 0,
    Pink,
    Blue,
    Yellow
}

bitflags! {
    #[derive(Clone, Copy, Debug)]
    pub struct SoaprunnerItems : u8 {
        const Sword = 1;
        const Crown = 2;
        const Shield = 4;
    }
}

pub const CLIENT_SPAWN_POSITION : Position = Position {
    x: 30,
    y: 22
};
pub const CLIENT_SPAWN_ROOM : RoomCoordinates = RoomCoordinates {
    x: 1,
    y: 1
};

#[derive(Clone, Debug)]
pub struct Soaprunner {
    //pub index: u8,
    pub teleport_trigger: u8,
    pub sprite: SoaprunnerSprites,
    pub color: SoaprunnerColors,
    pub items: SoaprunnerItems,
    //pub movements_length: u8,
    pub movements: Vec<Position>
}