use std::{fs, io};
use std::path::{Path, PathBuf};

use serde::{self, Deserialize, Serialize};
use thiserror::{self, Error};

use crate::soaprun::position::Position;
use crate::soaprun::units::UnitTypes;
use super::{Entity, EntityProperties, RoomVerificationBounds, RoomVerificationModes};

#[derive(serde::Deserialize, Debug)]
pub struct ServerConfig
{
    pub room_directory: PathBuf,
    pub room_verification_bounds: RoomVerificationBounds,
    pub room_verification_mode: RoomVerificationModes,
    pub entity_path: PathBuf,
    pub attributes_path: PathBuf,
    pub connection_timeout: u64,
    pub idle_timeout: u64,
    pub max_players: u32,
    pub max_player_movement_nodes_per_packet: u32,
    pub max_player_distance_per_movement_node: u32,
    pub max_player_distance_per_packet: u32,
    pub address: String
}


#[derive(Serialize, Deserialize)]
pub struct PositionInit {
    x: i16,
    y: i16,
}
#[derive(Serialize, Deserialize)]
pub struct FlameInit {
    x: i16,
    y: i16,
    #[serde(default = "default_flame_direction")]
    direction: u8
}
fn default_flame_direction() -> u8 { 0 }

#[derive(Serialize, Deserialize)]
pub struct GateInit {
    x: i16,
    y: i16,
    #[serde(default = "default_gate_direction")]
    open_direction: u8,
    switches: Vec<Position>
}
fn default_gate_direction() -> u8 { 2 }

#[derive(Serialize, Deserialize)]
#[serde(tag="type")]
pub enum EntityInitInfo {
    Goal(PositionInit),
    Closer(PositionInit),
    Sword(PositionInit),
    Crawl(PositionInit),
    Hummer(FlameInit),
    Rounder(FlameInit),
    Wuss(PositionInit),
    Chase(PositionInit),
    Gate(GateInit),
    Shield(PositionInit),
    Cross(FlameInit),
    Snail(PositionInit)
}

#[derive(Error, Debug)]
pub enum LoadEntityError {
    #[error("An error occured while loading the entity file: `{0}`")]
    FileLoadError(#[from] io::Error),
    #[error("An error occured while parsing the entity file: `{0}`")]
    DeserializeError(#[from] serde_json::Error)
}

pub fn load_entities(path: &Path) -> Result<Vec<Entity>,LoadEntityError> {
    let entity_str = fs::read_to_string(path)?;
    let entity_defs:Vec<EntityInitInfo> = serde_json::from_str(&entity_str)?;

    Ok(Vec::from_iter(entity_defs.iter().map(|e| {
        match e {
            EntityInitInfo::Goal(p) => {
                Entity::new(Position{ x: p.x, y: p.y }, UnitTypes::Goal, 0, EntityProperties::None)
            },
            EntityInitInfo::Closer(p) => {
                Entity::new(Position{ x: p.x, y: p.y }, UnitTypes::Closer, 0, EntityProperties::KillCounter(super::KillCounter { kills: 0 }))
            },
            EntityInitInfo::Sword(p) => {
                Entity::new(Position{ x: p.x, y: p.y }, UnitTypes::Sword, 0, EntityProperties::None)
            },
            EntityInitInfo::Crawl(p) => {
                Entity::new(Position{ x: p.x, y: p.y }, UnitTypes::Crawl, 0, EntityProperties::None)
            },
            EntityInitInfo::Hummer(f) => {
                Entity::new(Position{ x: f.x, y: f.y }, UnitTypes::Hummer,  f.direction, EntityProperties::None)
            },
            EntityInitInfo::Rounder(f) => {
                Entity::new(Position{ x: f.x, y: f.y }, UnitTypes::Rounder, f.direction, EntityProperties::None)
            },
            EntityInitInfo::Wuss(p) => {
                Entity::new(Position{ x: p.x, y: p.y }, UnitTypes::Wuss, 0, EntityProperties::KillCounter(super::KillCounter { kills: 0 }))
            },
            EntityInitInfo::Chase(p) => {
                Entity::new(Position{ x: p.x, y: p.y }, UnitTypes::Chase, 0, EntityProperties::None)
            },
            EntityInitInfo::Gate(g) => {
                Entity::new(Position{ x: g.x, y: g.y }, UnitTypes::Gate, 1, EntityProperties::SwitchedDirection( super::SwitchedDirection { off_dir: 1, on_dir: g.open_direction, switches: g.switches.clone() }))
            },
            EntityInitInfo::Shield(p) => {
                Entity::new(Position{ x: p.x, y: p.y }, UnitTypes::Shield, 0, EntityProperties::None)
            },
            EntityInitInfo::Cross(f) => {
                Entity::new(Position{ x: f.x, y: f.y }, UnitTypes::Cross, f.direction, EntityProperties::None)
            },
            EntityInitInfo::Snail(p) => {
                Entity::new(Position{ x: p.x, y: p.y }, UnitTypes::Snail, 0, EntityProperties::None)
            },
        }
    })))
}