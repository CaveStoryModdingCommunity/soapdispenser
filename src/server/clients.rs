use std::collections::HashSet;
use std::net::TcpStream;
use std::{collections::HashMap, time::Duration};
use std::sync::atomic::Ordering;

#[cfg(debug_assertions)]
use no_deadlocks::RwLockWriteGuard;
#[cfg(not(debug_assertions))]
use std::sync::RwLockWriteGuard;

use thiserror::Error;

use crate::soaprun::units::{UnitStates, UnitTypes};
use crate::soaprun::packets::*;
use crate::soaprun::soaprunners::*;
use crate::soaprun::position::Position;
use crate::soaprun::map_attributes::{DRAW_TILES, MAKE_CORPSE_TILES};

use super::map_attributes::CANVAS_TILES;
use super::position_extensions::DirectionFlags;
use super::{MAX_X_COORD, MAX_Y_COORD, MIN_X_COORD, MIN_Y_COORD, PROTOCOL_NAME, PROTOCOL_VERSION};
use super::{Entity, RoomCoordinates, SoaprunServer};


#[derive(Error, Debug)]
pub enum MovementValidationErrors {
    #[error("Nodes weren't aligned")]
    MisalignedNodesError,
    #[error("Tried to move {actual} tiles in one node (max is {max})")]
    NodesTooFarError {
        actual: usize,
        max: usize
    },
    #[error("Moved out of bounds")]
    OutOfBoundsError,
    #[error("Moved along an edge")]
    MoveAlongEdgeError,
    #[error("Moved onto the tile at {pos} which has type {tile_type}")]
    InvalidTileTypeError {
        pos: Position,
        tile_type: u8
    },
    #[error("Tried to move with {actual} nodes in one packet (max is {max})")]
    TooManyNodesError {
        actual: usize,
        max: usize
    },
    #[error("Tried to move {actual} tiles in one packet (max is {max})")]
    TotalTooFarError {
        actual: usize,
        max: usize
    },
    #[error("The first movement didn't end at spawn")]
    FirstMovementError
}

pub struct Client {
    pub number: usize,
    pub has_moved: bool,
    pub has_made_corpse: bool,
    pub kills: usize,
    pub claimed_sword: Option<usize>,
    pub claimed_shield: Option<usize>,
    pub room: HashSet<RoomCoordinates>,
    pub soaprunner: Soaprunner,
    pub cached_tiles: HashMap<RoomCoordinates,HashMap<Position, u8>>
}
impl Client {
    pub fn new(number: usize, color: SoaprunnerColors) -> Client
    {
        return Client {
            number: number,
            has_moved: false,
            has_made_corpse: false,
            kills: 0,
            claimed_sword: None,
            claimed_shield: None,
            room: HashSet::from([CLIENT_SPAWN_ROOM]),
            soaprunner: Soaprunner {
                teleport_trigger: 0,
                sprite: SoaprunnerSprites::Walking,
                color: color,
                items: SoaprunnerItems::empty(),
                movements: vec![CLIENT_SPAWN_POSITION]
            },
            cached_tiles: HashMap::new()
        }
    }

    pub fn verify_nodes(&self, p1 : &Position, p2 : &Position, context: &SoaprunServer) -> Result<usize,MovementValidationErrors> {
        if *p1 == *p2 {
            return Ok(0);
        }
        if !p1.in_line(p2) {
            return Err(MovementValidationErrors::MisalignedNodesError);
        }
        
        let dist = p1.taxicab_distance(p2);
        if context.max_player_distance_per_movement_node > 0 && dist > context.max_player_distance_per_movement_node {
            return Err(MovementValidationErrors::NodesTooFarError { actual:dist, max: context.max_player_distance_per_movement_node });
        }
        
        let dir_f = match p1.relative_direction(p2) {
            DirectionFlags::West => Position::west,
            DirectionFlags::East => Position::east,
            DirectionFlags::North => Position::north,
            DirectionFlags::South => Position::south,
            _ => unreachable!()
        };
        for i in 0..dist {
            let prev_pos = dir_f(&p1, i as i16);
            let curr_pos = dir_f(&p1, 1 + i as i16);
            //going outside of these bounds will softlock the client if we validate them
            if curr_pos.x < MIN_X_COORD || MAX_X_COORD < curr_pos.x 
            || curr_pos.y < MIN_Y_COORD || MAX_Y_COORD < curr_pos.y {
                return Err(MovementValidationErrors::OutOfBoundsError);
            }
            //multiple unique edge/corner movements in a row are not possible by the standard client
            if prev_pos.on_edge() && curr_pos.on_edge() {
                //TODO make this error part of the config
                return Err(MovementValidationErrors::MoveAlongEdgeError)
            }
            //ghosts go through everything, so we don't need to enter the loop
            //if the previous position was on an edge, the player is entering a new room, which is always allowed
            if !matches!(self.soaprunner.sprite, SoaprunnerSprites::Ghost) && !prev_pos.on_edge() {
                for rc in curr_pos.get_affected_rooms() {
                    //unwrap because we know each room is valid
                    let tile_type = context.get_tile_type(&curr_pos, &rc).unwrap();
                    if !self.can_move_on_tile_type(tile_type) {
                        return Err(MovementValidationErrors::InvalidTileTypeError { pos: curr_pos, tile_type });
                    }
                }
            }
        }
        return Ok(dist);
    }
    pub fn update_position(client: &mut Client, movements: &Vec<Position>, context: &SoaprunServer) -> Result<usize,MovementValidationErrors> {
        #[cfg(debug_assertions)]
        println!("{}: {} | {}",
            client.number,
            client.soaprunner.movements.iter().map(|p| { p.to_string() }).collect::<Vec<String>>().join(" -> "),
            movements.iter().map(|p| { p.to_string() }).collect::<Vec<String>>().join(" -> ")
        );
        if movements.len() <= 0 {
            return Ok(0)
        }
        let mut total = 0;
        match client.has_moved {
            true => {
                if context.max_player_movement_nodes_per_packet > 0 && movements.len() > context.max_player_movement_nodes_per_packet {
                    return Err(MovementValidationErrors::TooManyNodesError { actual: movements.len(), max: context.max_player_movement_nodes_per_packet });
                }
                total = client.verify_nodes(client.soaprunner.movements.last().unwrap(), &movements[0], context)?;
                for w in movements.windows(2) {
                    total += client.verify_nodes(&w[0], &w[1], context)?;
                }
                if context.max_player_distance_per_packet > 0 && total > context.max_player_distance_per_packet {
                    return Err(MovementValidationErrors::TotalTooFarError { actual: total, max: context.max_player_distance_per_packet });
                }
            }
            //when a client that has previously played is spawning, they may send their previous disconnect location before their spawn location
            //therefore, we only need to check that their final movement is in the right place, then never go here again
            false => {
                let dest = *movements.last().unwrap();
                if *client.soaprunner.movements.last().unwrap() == dest {
                    client.soaprunner.movements = vec![dest];
                    client.soaprunner.teleport_trigger = client.soaprunner.teleport_trigger.wrapping_add(1);
                    client.has_moved = true;
                } else {
                    return Err(MovementValidationErrors::FirstMovementError);
                }
            }
        }
        client.room = client.soaprunner.movements.last().unwrap().get_affected_rooms();
        //TODO remove this clone maybe?
        client.soaprunner.movements = movements.clone();
        Ok(total)
    }
    pub fn kill(mut client: RwLockWriteGuard<Self>) {
        client.soaprunner.sprite = SoaprunnerSprites::Dying;
    }
    pub fn add_kill(mut client: RwLockWriteGuard<Self>, context: &SoaprunServer) {
        client.kills += 1;
        if client.kills % 10 == 0 {
            client.soaprunner.items.insert(SoaprunnerItems::Crown);
            Self::return_sword(client, context);
        }
    }
    pub fn claim_sword(mut client: RwLockWriteGuard<Self>, sword_index: usize, context: &SoaprunServer) {
        if !client.soaprunner.items.contains(SoaprunnerItems::Sword) {
            //holding two write locks here SHOULD be ok, since swords will never be locked by anyone other than soaprunners
            let mut sword = match context.entities.get(sword_index) {
                Some(s) => s.write().unwrap(),
                None => return, //TODO return on invalid sword???
            };
            if matches!(sword.unit.unit_state, UnitStates::Active) {
                client.claimed_sword = Some(sword_index);
                client.soaprunner.items.insert(SoaprunnerItems::Sword);
                sword.unit.unit_state = UnitStates::Corpse;
            }
        }
    }
    pub fn return_sword(mut client: RwLockWriteGuard<Self>, context: &SoaprunServer) {
        if client.soaprunner.items.contains(SoaprunnerItems::Sword) {
            client.soaprunner.items.remove(SoaprunnerItems::Sword);
            let sword_index = client.claimed_sword.expect("Player had a sword without claiming one!");
            drop(client);
            let mut sword = context.entities.get(sword_index)
                .expect("Player claimed an invalid sword!").write().unwrap();
            sword.unit.unit_state = UnitStates::Active;
            sword.unit.teleport_trigger = sword.unit.teleport_trigger.wrapping_add(1);
        }
    }
    pub fn claim_shield(mut client: RwLockWriteGuard<Self>, shield_index: usize, context: &SoaprunServer) {
        if !client.soaprunner.items.contains(SoaprunnerItems::Shield) {
            //holding two write locks here SHOULD be ok, since swords will never be locked by anyone other than soaprunners
            let mut sword = match context.entities.get(shield_index) {
                Some(s) => s.write().unwrap(),
                None => return, //TODO return on invalid shield???
            };
            if matches!(sword.unit.unit_state, UnitStates::Active) {
                client.claimed_shield = Some(shield_index);
                context.players_with_shield.fetch_add(1, Ordering::Relaxed);
                client.soaprunner.items.insert(SoaprunnerItems::Shield);
                sword.unit.unit_state = UnitStates::Corpse;
            }
        }
    }
    pub fn drop_shield(mut client: RwLockWriteGuard<Self>, context: &SoaprunServer) {
        if client.soaprunner.items.contains(SoaprunnerItems::Shield) {
            context.players_with_shield.fetch_sub(1, Ordering::Relaxed);
            client.soaprunner.items.remove(SoaprunnerItems::Shield);
            let drop_pos = *client.soaprunner.movements.last().unwrap();
            let claimed_shield = client.claimed_shield.expect("Player had a shield without claiming one!");
            drop(client);

            let mut shield = context.entities.get(claimed_shield)
            .expect("Player claimed an invalid shield!").write().unwrap();
        
            //don't drop the shield on top of other entities
            if context.entities.iter().enumerate().any(|(n,e)| {
                n != claimed_shield
                && *e.read().unwrap().unit.movements.last().unwrap() == drop_pos
            }) {
                shield.unit.movements = vec![shield.spawn_position];
            }
            else {
                shield.unit.movements = vec![drop_pos];
            }
            shield.unit.unit_state = UnitStates::Active;
            shield.unit.teleport_trigger = shield.unit.teleport_trigger.wrapping_add(1);
        }
    }
    pub fn can_move_on_tile_type(&self, tile_type: u8) -> bool {
        matches!(self.soaprunner.sprite, SoaprunnerSprites::Ghost)
        || tile_type == 0
        || (tile_type == 2 && !self.soaprunner.items.contains(SoaprunnerItems::Shield))
    }
}

impl SoaprunServer {
    fn update_client_and_send_fields(&self, stream: &mut TcpStream, mut client: RwLockWriteGuard<Client>, movements: Vec<Position>) -> Result<bool, std::io::Error>
    {
        let valid = match Client::update_position(&mut client, &movements, self) {
            Ok(_) => true,
            Err(e) => {
                eprintln!("Player {} failed their movement: {} | {}\nReason: {}",
                client.number,
                client.soaprunner.movements.iter().map(|p| { p.to_string() }).collect::<Vec<String>>().join(" -> "),
                movements.iter().map(|p| { p.to_string() }).collect::<Vec<String>>().join(" -> "),
                e);
                client.soaprunner.sprite = SoaprunnerSprites::Dying;
                false
            }
        };
        let sprite = client.soaprunner.sprite;
        let color = client.soaprunner.color;
        let items = client.soaprunner.items;
        let num = client.number;
        let mut tiles = Vec::new();
        //TODO remove clone
        for r in client.room.clone() {
            if let Some(t) = client.cached_tiles.get_mut(&r) {
                tiles.extend(t.drain().map(|(p, tile)| { ChangedTile::new(p.x, p.y, tile) }))
            }
        }
        drop(client);

        let players = match self.players.read() {
            Ok(l) => l,
            Err(_) => return Err(std::io::Error::from(std::io::ErrorKind::AddrNotAvailable))
        };
        let packet = ServerPackets::Fields
        {
            client_state: sprite,
            client_color: color,
            client_items: items,
            weather: match self.players_with_shield.load(Ordering::Acquire) {
                0 => Weather::Clear,
                _ => Weather::Rainy
            },
            //TODO data copying might be slow, but dealing with locks during sending would be worse I think
            soaprunners: Vec::from_iter(players.iter().filter_map(|(n,p)| {
                if *n == num {
                    None
                }
                else {
                    Some((*n, p.read().unwrap().soaprunner.clone()))
                }
            }).take(CLIENT_MAX_PLAYERS)),
            entities: Vec::from_iter(self.entities.iter().enumerate().map(|(n,e)| {
                (n, e.read().unwrap().unit.clone())
            }).take(CLIENT_MAX_ENTITIES)),
            tiles: tiles
        };
        write_packet(stream, packet)?;
        Ok(valid)
    }

    //returns the number of tiles modified
    fn try_spawn_corpse(&self, pos: &Position) -> usize {
        
        self.try_update_tile(pos, &*MAKE_CORPSE_TILES, |b| { b + 16 })
    }
    //returns the number of tiles modified
    fn try_draw_on_field(&self, pos: &Position, tile: u8) -> usize
    {
        //invalid tile
        if !DRAW_TILES.contains(&tile) {
            return 0;
        }
        self.try_update_tile(pos, &*CANVAS_TILES, |t| { (t & 16) | tile })
    }
    
    fn handle_collision(&self, mut client: RwLockWriteGuard<Client>, entity_index: u8)
    {
        let colliding = match self.entities.get(entity_index as usize) {
            Some(e) => e,
            None => return //TODO invalid collisions are ignored for now
        };
        let colliding_r = colliding.read().unwrap();
        //TODO tighten this behavior up after entity behavior has been verified
        if colliding_r.unit.movements.last().unwrap().taxicab_distance(client.soaprunner.movements.last().unwrap()) > 15 {
            return
        }
        match colliding_r.unit.unit_type {
            UnitTypes::Goal => {
                client.soaprunner.sprite = SoaprunnerSprites::Winning
            },
            UnitTypes::Closer => {
                if matches!(colliding_r.unit.unit_state, UnitStates::Active) {
                    drop(colliding_r);
                    if client.soaprunner.items.contains(SoaprunnerItems::Sword) {
                        Client::add_kill(client, self);
                        Entity::kill(colliding.write().unwrap(),Duration::from_secs(5), self);
                    } else {
                        Client::kill(client);
                        Entity::add_kill(colliding.write().unwrap());
                    }
                }
            },
            UnitTypes::Sword => {
                drop(colliding_r);
                Client::claim_sword(client, entity_index as usize, self);
            },
            UnitTypes::Wuss => {
                if matches!(colliding_r.unit.unit_state, UnitStates::Active) {
                    Client::add_kill(client, self);
                    drop(colliding_r);
                    Entity::kill(colliding.write().unwrap(),Duration::from_secs(5), self);
                }
            },
            UnitTypes::Crawl => {
                if client.soaprunner.items.contains(SoaprunnerItems::Sword) {
                    Client::add_kill(client, self);
                    drop(colliding_r);
                    Entity::kill(colliding.write().unwrap(),Duration::from_secs(10), self);
                }
                else {
                    Client::kill(client);
                }
            },
            UnitTypes::Hummer | UnitTypes::Rounder |
            UnitTypes::Gate | UnitTypes::Cross => {
                if !client.soaprunner.items.contains(SoaprunnerItems::Shield) {
                    Client::kill(client);
                }
            },
            UnitTypes::Chase => {
                if matches!(colliding_r.unit.unit_state, UnitStates::Active) {
                    if client.soaprunner.items.contains(SoaprunnerItems::Sword) {
                        Client::add_kill(client, self);
                        drop(colliding_r);
                        Entity::kill(colliding.write().unwrap(), Duration::from_secs(5), self);
                    } else {
                        Client::kill(client);
                    }
                }
            },
            UnitTypes::Shield => {
                drop(colliding_r);
                Client::claim_shield(client, entity_index as usize, self);
            },
            UnitTypes::Snail => {
                //Rumor has it that the snail could be killed... those rumors are wrong (As of v0.432)
            },
        }
    }
    pub fn client_handler(&self, mut stream: TcpStream)
    {
        let (num, client) = match self.borrow_player() {
            Ok(n) => n,
            Err(_) => return,
        };

        println!("Welcome player {num}!");
        if let Ok(_) = write_packet(&mut stream, ServerPackets::Welcome)
        {
            loop {
                match read_packet(&mut stream)
                {
                    Ok(packet) => match packet
                    {
                        ClientPackets::ProtocolRequest { game_version } => {
                            println!("Player {num} is requesting the server Protocol from game version {game_version}");
                            if let Err(_) = write_packet(&mut stream, ServerPackets::Protocol {
                                protocol: *PROTOCOL_NAME,
                                version: PROTOCOL_VERSION
                            }) {
                                break
                            }
                        },
                        ClientPackets::ConnectionTest { data } => {
                            println!("Player {num} is testing their connection...");
                            if let Err(_) = write_packet(&mut stream, ServerPackets::ConnectionTest { data: data }) {
                                break
                            }
                        },
                        ClientPackets::LogDebugMessage { message } => {
                            println!("Debug message from player {num}: {message}");
                            if let Err(_) = write_packet(&mut stream, ServerPackets::Void) {
                                break
                            }
                        },
                        ClientPackets::MapAttributeRequest => {
                            println!("Player {num} wants the map attributes");
                            if let Err(_) = write_packet(&mut stream, ServerPackets::MapAttributesResponse {
                                map_attributes: &self.map_attributes
                            } ) {
                                break
                            }
                        },
                        ClientPackets::RoomRequest { coords } => {
                            println!("Player {num} wants the room at {coords}");
                            if let Some(room) = self.rooms.get(&coords) {
                                let mut cw = client.write().unwrap();
                                if let Some(cache) = cw.cached_tiles.get_mut(&coords) {
                                    cache.clear();
                                }
                                drop(cw);
                                let r = room.read().unwrap();
                                if let Err(_) = write_packet(&mut stream, ServerPackets::RoomResponse {
                                    coords: coords,
                                    room: &r
                                }) {
                                    break
                                }
                            }
                            else {
                                if let Err(_) = write_packet(&mut stream, ServerPackets::RoomResponse {
                                    coords: coords,
                                    room: &self.default_room
                                }) {
                                    break
                                }
                            }
                        },
                        ClientPackets::ChangeColor { color, movements } => {
                            println!("Player {num} wants to change color to {color}");
                            let mut cw = client.write().unwrap();
                            if matches!(cw.soaprunner.sprite, SoaprunnerSprites::Walking) { //idle players can't change color
                                cw.soaprunner.color = match color {
                                    0 => SoaprunnerColors::Green,
                                    1 => SoaprunnerColors::Pink,
                                    2 => SoaprunnerColors::Blue,
                                    3 => SoaprunnerColors::Yellow,
                                    _ => cw.soaprunner.color
                                };
                                match self.update_client_and_send_fields(&mut stream, cw, movements) {
                                    Ok(true) => {},
                                    Ok(false) | Err(_) => break,
                                }
                            }
                            else {
                                eprintln!("...but player {num} can't change color while {:?}!", cw.soaprunner.sprite);
                                break
                            }
                        },
                        ClientPackets::MyPosition { movements } => {
                            match self.update_client_and_send_fields(&mut stream, client.write().unwrap(), movements) {
                                Ok(true) => {},
                                Ok(false) | Err(_) => break,
                            }
                        },
                        ClientPackets::DrawOnField { position, tile, movements } => {
                            println!("Player {num} wants to change {position} to tile {tile}");
                            let state = client.read().unwrap().soaprunner.sprite;
                            if matches!(state, SoaprunnerSprites::Walking) { //idle players can't draw
                                let _ = self.try_draw_on_field(&position, tile);
                                match self.update_client_and_send_fields(&mut stream,  client.write().unwrap(), movements) {
                                    Ok(true) => {},
                                    Ok(false) | Err(_) => break,
                                }
                            }
                            else {
                                eprintln!("...but player {num} can't draw while {:?}!", state);
                                break
                            }
                        },
                        ClientPackets::HitNonPlayerUnit { index, movements } => {
                            println!("Player {num} has collided with entity {index}");
                            let state = client.read().unwrap().soaprunner.sprite;
                            if matches!(state, SoaprunnerSprites::Idle | SoaprunnerSprites::Walking) {
                                //aquiring two locks is a little annoying
                                //but we NEED to have control over when the write lock ends during collision to avoid deadlocks with entities
                                self.handle_collision(client.write().unwrap(), index);
                                match self.update_client_and_send_fields(&mut stream, client.write().unwrap(), movements) {
                                    Ok(true) => {},
                                    Ok(false) | Err(_) => break,
                                }
                            }
                            else {
                                eprintln!("...but player {num} can't collide while {:?}!", state);
                                break
                            }
                        },
                        ClientPackets::MakeCorpse { position } => {
                            println!("Player {num} wants to become a corpse at {position}");
                            let mut cw = client.write().unwrap();
                            match (cw.has_made_corpse, matches!(cw.soaprunner.sprite, SoaprunnerSprites::Dying)) {
                                (false, true) => {
                                    cw.has_made_corpse = true;
                                    drop(cw);
                                    //try_spawn_corpse needs write access to every client
                                    let _ = self.try_spawn_corpse(&position);
                                    if let Err(_) = write_packet(&mut stream, ServerPackets::Void) {
                                        break
                                    }
                                },
                                (false, false) => {
                                    eprintln!("...but player {num} isn't dead yet, they're {:?}!", cw.soaprunner.sprite);
                                    break
                                },
                                (true, false) => {
                                    eprintln!("...but player {num} already made a corpse!");
                                    break
                                },
                                (true, true) => {
                                    eprintln!("...but player {num} already made a corpse, and they're {:?}...?!", cw.soaprunner.sprite);
                                    break
                                },
                            }
                        },
                        ClientPackets::Bye => {
                            println!("Goodbye player {num}!");
                            let _ = write_packet(&mut stream, ServerPackets::Void);
                            break
                        },
                        ClientPackets::Heaven { movements } => {
                            println!("Player {num} would like to enter heaven");
                            let state = client.read().unwrap().soaprunner.sprite;
                            if matches!(state, SoaprunnerSprites::Idle | SoaprunnerSprites::Walking) {
                                Client::return_sword(client.write().unwrap(), self);
                                Client::drop_shield(client.write().unwrap(), self);
                                let mut cw = client.write().unwrap();
                                cw.soaprunner.sprite = SoaprunnerSprites::Ghost;
                                match self.update_client_and_send_fields(&mut stream, cw, movements) {
                                    Ok(true) => {},
                                    Ok(false) | Err(_) => break,
                                }
                            }
                            else {
                                eprintln!("...but player {num} can't enter heaven while {:?}!", state);
                                break
                            }
                        },
                    },
                    Err(e) => {
                        eprintln!("Error: {e}");
                        break;
                    }
                }
            }
        }
        Client::return_sword(client.write().unwrap(), self);
        Client::drop_shield(client.write().unwrap(), self);
        let _ = self.return_player(client);
    }
}