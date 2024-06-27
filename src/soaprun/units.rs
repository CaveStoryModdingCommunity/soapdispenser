use super::position::Position;

#[repr(u8)]
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum UnitStates {
    Sleeping = 0,
    Active,
    Corpse,
    Flickering,
    Gone
}
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum UnitTypes
{
    Goal = 0,
    Closer,
    Sword,
    Crawl,
    Hummer,
    Rounder,
    Wuss,
    Chase,
    Gate,
    Shield,
    Cross,
    Snail
}
#[derive(Debug, Clone)]
pub struct Unit {
    //pub index: u8,
    pub teleport_trigger: u8,
    pub unit_state: UnitStates,
    pub unit_type: UnitTypes,
    pub direction: u8,
    //pub movements_length: u8,
    pub movements: Vec<Position>
}