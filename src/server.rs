use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap, HashMap};
use std::net::{TcpListener, ToSocketAddrs};
use std::time::Duration;
use std::thread;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

#[cfg(debug_assertions)]
use no_deadlocks::{Mutex, RwLock};
#[cfg(not(debug_assertions))]
use std::sync::{Mutex, RwLock};

use rand::distributions::{Distribution, WeightedIndex};
use rand::thread_rng;
use thiserror::Error;

use crate::soaprun::packets::PROTOCOL_BUFFER_SIZE;
use crate::soaprun::map_attributes::MapAttributes;
use crate::soaprun::rooms::*;
use crate::soaprun::soaprunners::*;
use crate::soaprun::*;

mod config;
pub use config::*;
mod rooms;
use rooms::*;
pub use rooms::{ROOM_EXTENSION, ROOM_COORD_SEPARATOR};
mod clients;
use clients::*;
mod entities;
use entities::*;
mod position_extensions;

pub const PROTOCOL_NAME : &[u8; PROTOCOL_BUFFER_SIZE] = b"Soaprun\0";
pub const PROTOCOL_VERSION : u16 = 64;

pub struct SoaprunServer
{
    //individual rooms need to be locked when tiles are updated
    rooms: HashMap<RoomCoordinates, RwLock<Room>>,
    //the default room and map attributes never change, so no lock is needed
    default_room: Room,
    map_attributes: MapAttributes,
    
    players_with_shield: AtomicUsize,

    entity_update_rate: Duration,
    //the number of entities is fixed, so we never need to lock the collection as a whole, just the elements
    entities: Vec<RwLock<Entity>>,

    max_player_movement_nodes_per_packet: usize,
    max_player_distance_per_movement_node: usize,
    max_player_distance_per_packet: usize,

    //player number heap is only accessed during joins/leaves, so mutex it is
    player_numbers: Mutex<BinaryHeap<Reverse<usize>>>,
    //the entire player list is only locked during joins/leaves
    //individual players may be locked frequently to update their state
    players: RwLock<BTreeMap<usize, Arc<RwLock<Client>>>>
}
#[derive(Error, Debug)]
pub enum NewServerError {
    #[error("An error occured while loading a room: `{0}`")]
    LoadRoomError(#[from] LoadRoomError),
    #[error("`{0}`")]
    RoomVerificationError(String),
    #[error("An error occured while loading the map attributes: `{0}`")]
    MapAttributesError(#[from] std::io::Error),
    #[error("An error occured while loading the entities: `{0}`")]
    EntityLoadError(#[from] LoadEntityError)
}
impl SoaprunServer
{
    pub fn new(config: &ServerConfig) -> Result<&'static SoaprunServer, NewServerError>
    {
        let mut pn = BinaryHeap::with_capacity(config.max_players as usize);
        for i in 0..config.max_players {
            pn.push(Reverse(i as usize));
        }

        let mut rooms = load_rooms(&config.room_directory)?;
        println!("Loaded {} rooms with coordinates {}", rooms.len(), rooms.iter().map(|(c, _)| { c.to_string() }).collect::<Vec<String>>().join(", ") );
        
        let default_room = match Room::new(config.room_directory.join(DEFAULT_ROOM_NAME)) {
            Ok(r) => r,
            Err(e) => return Err(NewServerError::LoadRoomError(LoadRoomError::NewRoomError(e))),
        };
        println!("...Plus the default room");
        let map_attributes = MapAttributes::new(&config.attributes_path)?;
        println!("Loaded map attributes");

        if let Err(e) = verify_rooms(&rooms, &default_room, &config.room_verification_bounds, match config.room_verification_mode {
            RoomVerificationModes::Tiles => None,
            RoomVerificationModes::TileTypes => Some(&map_attributes),
        }) {
            return Err(NewServerError::RoomVerificationError(e))
        }
        println!("Verified rooms");

        let rooms = HashMap::from_iter(rooms.drain().map(|(c,r)| {
            (c,RwLock::new(r))
        }));

        let mut entities = load_entities(&config.entity_path)?;
        println!("Loaded {} entities", entities.len());

        let entities = Vec::from_iter(entities.drain(0..).map(|e| {
            RwLock::new(e)
        }));

        let server = Box::new(SoaprunServer
            {
                player_numbers: Mutex::new(pn),
                players:  RwLock::new(BTreeMap::new()),
                
                entity_update_rate: Duration::from_millis(10),
                entities: entities,

                players_with_shield: AtomicUsize::new(0),

                max_player_movement_nodes_per_packet: config.max_player_movement_nodes_per_packet as usize,
                max_player_distance_per_movement_node: config.max_player_distance_per_movement_node as usize,
                max_player_distance_per_packet: config.max_player_distance_per_packet as usize,
                
                rooms: rooms,
                default_room: default_room,

                map_attributes: map_attributes
            });
        Ok(Box::leak(server))
    }
    fn get_player_color(&self) -> SoaprunnerColors {
        let choices = [SoaprunnerColors::Green, SoaprunnerColors::Pink, SoaprunnerColors::Blue, SoaprunnerColors::Yellow];
        //TODO put in config
        let weights = [1,1,1,1];
        let dist = WeightedIndex::new(&weights).unwrap();
        choices[dist.sample(&mut thread_rng())]
    }
    fn borrow_player(&self) -> Result<(usize, Arc<RwLock<Client>>), ()>
    {
        let num = match self.player_numbers.lock().unwrap().pop() {
            Some(num) => num.0,
            None => return Err(()),
        };
        let client = Arc::new(RwLock::new(Client::new(num, self.get_player_color())));
        self.players.write().unwrap().insert(num, client.clone());
        Ok((num, client))
    }
    fn return_player(&self, client: Arc<RwLock<Client>>) -> Result<(),()>
    {
        let num = client.read().unwrap().number;
        match self.players.write().unwrap().remove(&num) {
            Some(_) => println!("Removed player {num}"),
            None => println!("Player wasn't in the list...?!"),
        };
        self.player_numbers.lock().unwrap().push(Reverse(num));
        drop(client);
        Ok(())
    }
    pub fn start_server<A>(&'static self, address: A) -> Result<(), std::io::Error>
        where A : ToSocketAddrs
    {
        let listener = TcpListener::bind(address)?;
        let _ = thread::spawn(|| { //TODO maybe close this thread properly on exit
            self.entity_handler()
        });
        println!("Listening on {}", listener.local_addr().unwrap());
        for acc_res in listener.incoming() {
            match acc_res
            {
                Ok(stream) => {
                    thread::spawn(|| {
                        self.client_handler(stream)
                    });
                },
                Err(e) => {
                    eprintln!("Error accepting incoming connection: {:?}", e)
                }
            }
        }
        return Ok(());
    }
}