use std::{collections::HashSet, thread::sleep, time::Duration};
use std::sync::atomic::Ordering;

#[cfg(debug_assertions)]
use no_deadlocks::{RwLock, RwLockWriteGuard};
#[cfg(not(debug_assertions))]
use std::sync::{RwLock, RwLockWriteGuard};

use crate::soaprun::position::Position;
use crate::soaprun::units::{Unit, UnitStates, UnitTypes};
use crate::soaprun::soaprunners::{SoaprunnerSprites, SoaprunnerItems};
use rand::{seq::SliceRandom, thread_rng};

use super::map_attributes::REMOVE_CORPSE_TILES;
use super::position_extensions::DirectionFlags;
use super::SoaprunServer;

pub struct KillCounter {
    pub kills: usize
}
pub struct SwitchedDirection {
    pub off_dir: u8,
    pub on_dir: u8,
    pub switches: Vec<Position>
}

pub enum EntityProperties {
    None,
    KillCounter(KillCounter),
    SwitchedDirection(SwitchedDirection)
}
pub struct Entity {
    pub spawn_position: Position,
    pub counter: usize,
    pub properties: EntityProperties,
    pub unit: Unit,
}
impl Entity {
    pub fn new(pos: Position, unit_type: UnitTypes, direction: u8, properties: EntityProperties) -> Entity {
        return Entity {
            spawn_position: pos,
            counter: 0,
            properties: properties,
            unit: Unit {
                teleport_trigger: 0,
                unit_state: match unit_type {
                    UnitTypes::Goal => UnitStates::Active,
                    UnitTypes::Closer => UnitStates::Sleeping,
                    UnitTypes::Sword => UnitStates::Active,
                    UnitTypes::Crawl => UnitStates::Active, //There is no footage of a Crawl respawning, so I assume they never slept
                    UnitTypes::Hummer => UnitStates::Active,
                    UnitTypes::Rounder => UnitStates::Active,
                    UnitTypes::Wuss => UnitStates::Sleeping,
                    UnitTypes::Chase => UnitStates::Sleeping,
                    UnitTypes::Gate => UnitStates::Active,
                    UnitTypes::Shield => UnitStates::Active,
                    UnitTypes::Cross => UnitStates::Active,
                    UnitTypes::Snail => UnitStates::Sleeping,
                },
                unit_type: unit_type,
                direction,
                movements: vec![pos],
            },
        }
    }
    pub fn kill(mut unit: RwLockWriteGuard<Self>, dead_len: Duration, context: &SoaprunServer) {
        unit.unit.unit_state = UnitStates::Corpse;
        unit.counter = context.get_entity_delay(dead_len);
        if matches!(unit.unit.unit_type, UnitTypes::Wuss) {
            unit.unit.unit_type = UnitTypes::Closer;
        }
        match &mut unit.properties {
            EntityProperties::KillCounter (kc) => kc.kills = 0,
            _ => { },
        };
        drop(unit);
    }
    pub fn add_kill(mut unit: RwLockWriteGuard<Self>) {
        if matches!(unit.unit.unit_type, UnitTypes::Closer) {
            match &mut unit.properties {
                EntityProperties::KillCounter(kc) => {
                    kc.kills += 1;
                    if kc.kills == 3 {
                        kc.kills = 0;
                        unit.unit.unit_type = UnitTypes::Wuss;
                    }
                },
                _ => unreachable!()
            }
        }
    }
    pub fn wait(mut unit: RwLockWriteGuard<Self>) -> Option<RwLockWriteGuard<Self>> {
        unit.counter = unit.counter.saturating_sub(1);
        if unit.counter == 0 {
            Some(unit)
        } else {
            None
        }
    }
    pub fn can_move_on_tile_type(tile_type: u8) -> bool {
        tile_type == 0 || tile_type == 3
    }
}

impl SoaprunServer {
    fn get_entity_delay(&self, wait_time: Duration) -> usize {
        (wait_time.as_millis()/self.entity_update_rate.as_millis()) as usize
    }
    fn get_invalid_tile_movements<TC>(&self, pos: Position, cmp: TC) -> DirectionFlags
        where TC : Fn(u8) -> bool
    {
        let mut flags = DirectionFlags::empty();

        let w = pos.west(1);
        if w.get_affected_rooms().iter().any(|rc| {
            !cmp(self.get_tile_type(&w, rc).unwrap())
         }) {
            flags.insert(DirectionFlags::West);
         }

        let n = pos.north(1);
        if n.get_affected_rooms().iter().any(|rc| {
            !cmp(self.get_tile_type(&n, rc).unwrap())
         }) {
            flags.insert(DirectionFlags::North);
         }

         let e = pos.east(1);
         if e.get_affected_rooms().iter().any(|rc| {
            !cmp(self.get_tile_type(&e, rc).unwrap())
         }) {
            flags.insert(DirectionFlags::East);
         }

         let s = pos.south(1);
         if s.get_affected_rooms().iter().any(|rc| {
            !cmp(self.get_tile_type(&s, rc).unwrap())
         }) {
            flags.insert(DirectionFlags::South);
         }

        flags
    }
    
    fn get_invalid_entity_movements(&self, pos: Position) -> DirectionFlags {
        let mut dir = DirectionFlags::empty();
        for e in self.entities.iter() {
            let ep = *e.read().unwrap().unit.movements.last().unwrap();
            if pos.adjacent_exclusive(&ep) {
                dir |= pos.relative_direction(&ep)
            }
        }
        dir
    }

    fn get_closer_movement_options(&self, unit: &RwLock<Entity>) -> Option<Vec<Position>> {
        let entity_r = unit.read().unwrap();
        let pos = *entity_r.unit.movements.last().unwrap();
        let spawn_pos = entity_r.spawn_position;
        let scared = matches!(entity_r.unit.unit_type, UnitTypes::Wuss);
        drop(entity_r);
        let invalid_dirs = self.get_invalid_tile_movements(pos, Entity::can_move_on_tile_type) | self.get_invalid_entity_movements(pos);
        
        const CLOSER_RADIUS: i16 = 3;
        let w = pos.x.saturating_sub(CLOSER_RADIUS);
        let n = pos.y.saturating_sub(CLOSER_RADIUS);
        let e = pos.x.saturating_add(CLOSER_RADIUS);
        let s = pos.y.saturating_add(CLOSER_RADIUS);

        let mut predators = Vec::new();
        let mut prey = Vec::new();

        let players = self.players.read().unwrap();
        for (_, p) in players.iter() {
            let pr = p.read().unwrap();
            if matches!(pr.soaprunner.sprite, SoaprunnerSprites::Idle | SoaprunnerSprites::Walking) {
                let pp = pr.soaprunner.movements.last().unwrap();
                if n <= pp.y && pp.y <= s
                && w <= pp.x && pp.x <= e {
                    if scared || pr.soaprunner.items.intersects(SoaprunnerItems::Sword | SoaprunnerItems::Crown) {
                        predators.push(*pp);
                    } else {
                        prey.push(*pp)
                    }
                }
            }
        }

        match (!predators.is_empty(), !prey.is_empty()) {
            //no one close, go home
            (false, false) => {
                if pos != spawn_pos {
                    let mut spawn_dir = pos.relative_direction(&spawn_pos);
                    spawn_dir &= !invalid_dirs;
                    //if we can't move towards spawn, move in a random direction
                    if spawn_dir.is_empty() {
                        spawn_dir = !invalid_dirs;
                    }
                    Some(spawn_dir.to_positions(&pos))
                }
                else {
                    None
                }
            },
            //run from predators
            (true, false) => {
                let mut run_dirs = DirectionFlags::empty();
                for p in predators {
                    run_dirs |= !pos.relative_direction(&p);
                }
                run_dirs &= !invalid_dirs;
                Some(run_dirs.to_positions(&pos))
            }, 
            //attack closest prey
            (_, true) => {
                let mut prey_dirs = DirectionFlags::empty();
                for p in prey {
                    prey_dirs |= pos.relative_direction(&p);
                }

                for p in predators {
                    if pos.adjacent_exclusive(&p) {
                        prey_dirs &= !pos.relative_direction(&p)
                    }
                }
                prey_dirs &= !invalid_dirs;
                Some(prey_dirs.to_positions(&pos))
            },
        }
    }
    fn get_crawl_attack_locations(&self, pos: Position) -> Vec<Position> {
        let mut include_flags = self.get_invalid_tile_movements(pos, Entity::can_move_on_tile_type);
        let mut adj_positions = Vec::with_capacity(4);

        if include_flags.is_all() {
            return adj_positions
        }

        let players = self.players.read().unwrap();
        for (_, p) in players.iter() {
            if include_flags.is_all() {
                break
            }

            let player_pos = {
                let pr = p.read().unwrap();
                if matches!(pr.soaprunner.sprite, SoaprunnerSprites::Idle | SoaprunnerSprites::Walking) {
                    *pr.soaprunner.movements.last().unwrap()
                } else {
                    continue
                }
            };

            if include_flags & DirectionFlags::Vertical != DirectionFlags::Vertical
            && player_pos.x == pos.x {
                if !include_flags.contains(DirectionFlags::North)
                && player_pos.y == pos.y - 1 {
                    //up enabled
                    include_flags.insert(DirectionFlags::North);
                    adj_positions.push(pos.north(1));
                } else if !include_flags.contains(DirectionFlags::South)
                && player_pos.y == pos.y + 1 {
                    //down enabled
                    include_flags.insert(DirectionFlags::South);
                    adj_positions.push(pos.south(1));
                }
            } else if (include_flags & DirectionFlags::Horizontal) != DirectionFlags::Horizontal
            && (pos.y - 1 <= player_pos.y && player_pos.y <= pos.y + 1) {
                if !include_flags.contains(DirectionFlags::West)
                && player_pos.x == pos.x - 1 {
                    //left allowed
                    include_flags.insert(DirectionFlags::West);
                    adj_positions.push(pos.west(1));
                } else if !include_flags.contains(DirectionFlags::East)
                && player_pos.x == pos.x + 1 {
                    //right allowed
                    include_flags.insert(DirectionFlags::East);
                    adj_positions.push(pos.east(1));
                }
            }
        }
        adj_positions
    }
    
    
    fn get_chase_movement_options(&self, pos: Position) -> Vec<Position> {
        let mut targets = Vec::with_capacity(self.players_with_shield.load(Ordering::Relaxed));

        let players = self.players.read().unwrap();
        for (_, p) in players.iter() {
            let pr = p.read().unwrap();
            if matches!(pr.soaprunner.sprite, SoaprunnerSprites::Idle | SoaprunnerSprites::Walking)
            && pr.soaprunner.items.contains(SoaprunnerItems::Shield) {
                targets.push(*pr.soaprunner.movements.last().unwrap())
            }
        }

        let invalid_moves = self.get_invalid_tile_movements(pos, Entity::can_move_on_tile_type) | self.get_invalid_entity_movements(pos);
        let mut valid_moves = DirectionFlags::empty();

        for t in targets {
            valid_moves |= pos.relative_direction(&t);
        }
        valid_moves &= !invalid_moves;
        valid_moves.to_positions(&pos)
    }
    
    fn get_snail_movement_options(&self, pos: Position, radius: i16) -> Option<Vec<Position>> {
        let invalid_dirs = self.get_invalid_tile_movements(pos, Entity::can_move_on_tile_type);
        let mut valid_dirs = DirectionFlags::empty();

        let w = pos.x.saturating_sub(radius);
        let n = pos.y.saturating_sub(radius);
        let e = pos.x.saturating_add(radius);
        let s = pos.y.saturating_add(radius);

        let players = self.players.read().unwrap();
        for (_, p) in players.iter() {
            let pr = p.read().unwrap();
            if matches!(pr.soaprunner.sprite, SoaprunnerSprites::Idle | SoaprunnerSprites::Walking) {
                let pp = pr.soaprunner.movements.last().unwrap();
                if n <= pp.y && pp.y <= s
                && w <= pp.x && pp.x <= e {
                    let dir = pos.relative_direction(pp);
                    valid_dirs.insert(dir)
                }
            }
        }

        if valid_dirs.is_empty() {
            None
        } else {
            valid_dirs &= !invalid_dirs;
            Some(valid_dirs.to_positions(&pos))
        }
    }
    
    pub fn entity_handler(&self) {
        loop {
            for unit in &self.entities {
                let entity_r = unit.read().unwrap();
                //anything with => { } doesn't move/need to be updated here
                match entity_r.unit.unit_type {
                    //these don't do anything, so...
                    UnitTypes::Goal | UnitTypes::Sword | UnitTypes::Shield |
                    UnitTypes::Hummer | UnitTypes::Rounder => { },
                    //these are basically the same enemy, so shared case it is
                    UnitTypes::Closer | UnitTypes::Wuss => {
                        match entity_r.unit.unit_state {
                            UnitStates::Sleeping | UnitStates::Active => {
                                drop(entity_r);
                                if let Some(entity_w) = Entity::wait(unit.write().unwrap()) {
                                    let curr_pos = *entity_w.unit.movements.last().unwrap();
                                    drop(entity_w);
                                    let options = self.get_closer_movement_options(unit);
                                    
                                    let mut entity_w = unit.write().unwrap();
                                    match options {
                                        Some(opts) => {
                                            if let Some(new_pos) = opts.choose(&mut thread_rng()) {
                                                entity_w.unit.movements = vec![curr_pos, *new_pos];
                                            } else {
                                                entity_w.unit.movements = vec![curr_pos];
                                            }
                                            entity_w.unit.unit_state = UnitStates::Active;
                                        },
                                        None => {
                                            entity_w.unit.unit_state = UnitStates::Sleeping;
                                            entity_w.unit.movements = vec![curr_pos];
                                        }
                                    }
                                    entity_w.counter = self.get_entity_delay(Duration::from_millis(500));
                                }
                            },
                            UnitStates::Corpse => {
                                drop(entity_r);
                                if let Some(mut entity_w) = Entity::wait(unit.write().unwrap()) {
                                    entity_w.unit.unit_state = UnitStates::Sleeping;
                                    entity_w.unit.teleport_trigger = entity_w.unit.teleport_trigger.wrapping_add(1);
                                    entity_w.unit.movements = vec![entity_w.spawn_position];
                                    entity_w.counter = self.get_entity_delay(Duration::from_secs(1));
                                }
                            },
                            UnitStates::Flickering => {},
                            UnitStates::Gone => {},
                        }
                    },
                    UnitTypes::Crawl => {
                        match entity_r.unit.unit_state {
                            UnitStates::Sleeping => {
                                //I have yet to see evidence of a sleeping Crawl, so this is a failsafe
                                drop(entity_r);
                                unit.write().unwrap().unit.unit_state = UnitStates::Active
                            },
                            UnitStates::Active => {
                                let last_pos = *entity_r.unit.movements.last().unwrap();
                                let spawn_pos = entity_r.spawn_position;
                                drop(entity_r);

                                if let Some(mut entity_w) = Entity::wait(unit.write().unwrap()) {
                                    if last_pos != spawn_pos {
                                        entity_w.unit.movements = vec![last_pos, spawn_pos];
                                        entity_w.counter = self.get_entity_delay(Duration::from_secs(1));
                                    }
                                    else {
                                        //don't hold a write lock while checking the players
                                        drop(entity_w);
                                        let targets = self.get_crawl_attack_locations(last_pos);
                                        if let Some(attack_pos) = targets.choose(&mut rand::thread_rng()) {
                                            let mut entity_w = unit.write().unwrap();
                                            entity_w.unit.movements = vec![last_pos, *attack_pos];
                                            entity_w.counter = self.get_entity_delay(Duration::from_secs(1));
                                        }
                                    }
                                }
                            },
                            UnitStates::Corpse => {
                                drop(entity_r);
                                if let Some(mut entity_w) = Entity::wait(unit.write().unwrap()) {
                                    entity_w.unit.unit_state = UnitStates::Active;
                                    entity_w.unit.teleport_trigger = entity_w.unit.teleport_trigger.wrapping_add(1);
                                    entity_w.unit.movements = vec![entity_w.spawn_position];
                                    entity_w.counter = self.get_entity_delay(Duration::from_secs(1));
                                }
                            },
                            UnitStates::Flickering => { },
                            UnitStates::Gone => { },
                        };
                    },
                    UnitTypes::Chase => {
                        match entity_r.unit.unit_state {
                            UnitStates::Sleeping => {
                                drop(entity_r);
                                if let Some(mut entity_w) = Entity::wait(unit.write().unwrap()) {
                                    if self.players_with_shield.load(Ordering::Relaxed) > 0 {
                                        entity_w.unit.unit_state = UnitStates::Active;
                                    }
                                }
                            },
                            UnitStates::Active => {
                                if self.players_with_shield.load(Ordering::Relaxed) > 0 {
                                    let pos = *entity_r.unit.movements.last().unwrap();
                                    drop(entity_r);
                                    let options = self.get_chase_movement_options(pos);
                                    let mut entity_w = unit.write().unwrap();
                                    if let Some(opt) = options.choose(&mut thread_rng()) {
                                        entity_w.unit.movements = vec![pos, *opt];
                                    } else {
                                        entity_w.unit.movements = vec![pos];
                                    }
                                } else {
                                    drop(entity_r);
                                    let mut entity_w = unit.write().unwrap();
                                    entity_w.unit.unit_state = UnitStates::Sleeping;
                                    entity_w.unit.movements = vec![*entity_w.unit.movements.last().unwrap()]
                                }
                            },
                            UnitStates::Corpse => {
                                drop(entity_r);
                                if let Some(mut entity_w) = Entity::wait(unit.write().unwrap()) {
                                    entity_w.unit.unit_state = UnitStates::Sleeping;
                                    entity_w.unit.teleport_trigger = entity_w.unit.teleport_trigger.wrapping_add(1);
                                    entity_w.counter = self.get_entity_delay(Duration::from_secs(5));
                                    entity_w.unit.movements = vec![entity_w.spawn_position];
                                }
                            },
                            UnitStates::Flickering => { },
                            UnitStates::Gone => { },
                        }
                    },
                    UnitTypes::Gate => {
                        let mut set: HashSet<Position> = HashSet::from_iter(match &entity_r.properties {
                            EntityProperties::SwitchedDirection(sd) => sd.switches.clone(),
                            _ => unreachable!()
                        });
                        drop(entity_r);

                        if let Some(entity_w) = Entity::wait(unit.write().unwrap()) {
                            drop(entity_w);

                            for (_, p) in self.players.read().unwrap().iter() {
                                if set.is_empty() {
                                    break
                                }
                                let pr = p.read().unwrap();
                                if matches!(pr.soaprunner.sprite, SoaprunnerSprites::Idle | SoaprunnerSprites::Walking) {
                                    let pp = pr.soaprunner.movements.last().unwrap();
                                    set.remove(pp);
                                }
                            }

                            let mut entity_w = unit.write().unwrap();
                            let prop = match &entity_w.properties {
                                EntityProperties::SwitchedDirection(sd) => sd,
                                _ => unreachable!()
                            };
                            if set.is_empty() {
                                entity_w.unit.direction = prop.on_dir;
                                entity_w.counter = self.get_entity_delay(Duration::from_secs(5))
                            } else {
                                entity_w.unit.direction = prop.off_dir;
                            }
                        }
                    },
                    UnitTypes::Cross => {
                        drop(entity_r);
                        if let Some(mut entity_w) = Entity::wait(unit.write().unwrap()) {
                            entity_w.unit.direction = entity_w.unit.direction.wrapping_add(1) % 4;
                            entity_w.counter = self.get_entity_delay(Duration::from_secs(10));
                        }
                    },
                    UnitTypes::Snail => {
                        match entity_r.unit.unit_state {
                            UnitStates::Sleeping | UnitStates::Active => {
                                let pos = *entity_r.unit.movements.last().unwrap();
                                let radius = match entity_r.unit.unit_state {
                                    UnitStates::Sleeping => 1,
                                    UnitStates::Active => 2,
                                    _ => unreachable!()
                                };
                                let _ = self.try_update_tile(&pos, &*REMOVE_CORPSE_TILES, |t| { t - 16 });
                                drop(entity_r);

                                if let Some(entity_w) = Entity::wait(unit.write().unwrap()) {
                                    drop(entity_w);

                                    let options = self.get_snail_movement_options(pos, radius);
                                    let mut entity_w = unit.write().unwrap();
                                    match options {
                                        Some(o) => {
                                            entity_w.unit.unit_state = UnitStates::Active;
                                            if let Some(new_pos) = o.choose(&mut thread_rng()) {
                                                entity_w.unit.movements = vec![pos, *new_pos];
                                            }
                                            entity_w.counter = self.get_entity_delay(Duration::from_secs(1));
                                        },
                                        None => {
                                            entity_w.unit.unit_state = UnitStates::Sleeping;
                                            if entity_w.unit.movements.len() > 1 {
                                                entity_w.unit.movements = vec![pos];
                                            }
                                        },
                                    }
                                }
                            },
                            UnitStates::Corpse => {
                                drop(entity_r);
                                if let Some(mut entity_w) = Entity::wait(unit.write().unwrap()) {
                                    entity_w.unit.unit_state = UnitStates::Sleeping;
                                    entity_w.unit.teleport_trigger = entity_w.unit.teleport_trigger.wrapping_add(1);
                                    entity_w.unit.movements = vec![entity_w.spawn_position];
                                    entity_w.counter = self.get_entity_delay(Duration::from_secs(1));
                                }
                            },
                            UnitStates::Flickering => { },
                            UnitStates::Gone => { },
                        }
                    },
                }
            }
            sleep(self.entity_update_rate);
        }
    }
}