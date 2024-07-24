use std::{io::Write, net::TcpStream};
use std::io::{Error, ErrorKind, Read};

use thiserror::Error;

use super::units::Unit;
use super::map_attributes::MapAttributes;
use super::position::Position;
use super::rooms::*;
use super::soaprunners::*;

pub const CONNECTION_TEST_DATA_SIZE : usize = 508;
pub enum ClientPackets {
    ProtocolRequest {
        game_version: u16
    },
    MapAttributeRequest,
    RoomRequest {
        coords: RoomCoordinates
    },
    MyPosition {
        movements: Vec<Position>
    },
    MakeCorpse {
        position: Position
    },
    ConnectionTest {
        data: [u8; CONNECTION_TEST_DATA_SIZE]
    },
    LogDebugMessage {
        message: String
    },
    Bye,
    HitNonPlayerUnit {
        index: u8,
        movements: Vec<Position>
    },
    Heaven {
        movements: Vec<Position>
    },
    ChangeColor {
        color: u8,
        movements: Vec<Position>
    },
    DrawOnField {
        position: Position,
        tile: u8,
        movements: Vec<Position>
    },
}

pub const PACKET_TYPE_PROTOCOL            : [u8; 4] = *b"Prtc";
pub const PACKET_TYPE_MAP_ATTRIBUTES      : [u8; 4] = *b"mAtt";
pub const PACKET_TYPE_ROOM                : [u8; 4] = *b"Room";
pub const PACKET_TYPE_MY_POSITION         : [u8; 4] = *b"myPo";
pub const PACKET_TYPE_MAKE_CORPSE         : [u8; 4] = *b"mCrp";
pub const PACKET_TYPE_TEST                : [u8; 4] = *b"Test";
pub const PACKET_TYPE_DEBUG_LOG           : [u8; 4] = *b"Dlog";
pub const PACKET_TYPE_BYE                 : [u8; 4] = *b"Bye.";
pub const PACKET_TYPE_HIT_NON_PLAYER_UNIT : [u8; 4] = *b"HNPU";
pub const PACKET_TYPE_HEAVEN              : [u8; 4] = *b"HVen";
pub const PACKET_TYPE_CHANGE_COLOR        : [u8; 4] = *b"ChCl";
pub const PACKET_TYPE_DRAW_ON_FIELD       : [u8; 4] = *b"DrFl";


pub const PROTOCOL_BUFFER_SIZE : usize = 8;
pub const CLIENT_MAX_PLAYERS : usize = 63;
pub const CLIENT_MAX_ENTITIES : usize = 64;


#[repr(u8)]
#[derive(Clone, Copy)]
pub enum Weather {
    Clear = 0,
    Rainy
}
#[derive(Debug)]
pub struct ChangedTile {
    pub x: i16,
    pub y: i16,
    pub tile: u8,
    padding: u8
}
impl ChangedTile {
    pub fn new(x: i16, y: i16, tile: u8) -> ChangedTile {
        ChangedTile {
            x, y, tile,
            padding: 0
        }
    }
}
pub enum ServerPackets<'a> {
    Welcome,
    Protocol {
        protocol: [u8; PROTOCOL_BUFFER_SIZE],
        version: u16
    },
    MapAttributesResponse {
        map_attributes: &'a MapAttributes
    },
    RoomResponse {
        coords: RoomCoordinates,
        room: &'a Room,
    },
    Fields {
        client_state: SoaprunnerSprites,
        client_color: SoaprunnerColors,
        client_items: SoaprunnerItems,
        //soaprunners_length: u8,
        //entities_length: u8,
        //tiles_length: u8,
        weather: Weather,
        soaprunners: Vec<(usize, Soaprunner)>,
        entities: Vec<(usize, Unit)>,
        tiles: Vec<ChangedTile>
    },
    ConnectionTest {
        data: [u8; CONNECTION_TEST_DATA_SIZE]
    },
    Void,
}
pub const PACKET_TYPE_WELCOME : [u8; 4] = *b"WLCM";
pub const PACKET_TYPE_FIELDS  : [u8; 4] = *b"Flds";
pub const PACKET_TYPE_VOID    : [u8; 4] = *b"Void";


const MIN_PACKET_LENGTH : usize = 4;
#[derive(Error, Debug)]
pub enum ReadPacketErrors
{
    #[error("Invalid packet length: {length}")]
    InvalidLengthError {
        length: u32
    },
    #[error("Invalid packet type: {:?}", chars)]
    InvalidTypeError {
        chars: Vec<u8>
    },
    #[error("IO error while receiving packet: `{0}`", )]
    IOError(#[from] std::io::Error),
    #[error("Unexpected data amount: got {got} bytes when {expected} were expected")]
    UnexpectedDataAmount {
        got: usize,
        expected: usize
    },
    #[error("The provided data was invalid for type {packet_type}. {:?}", data)]
    InvalidDataError {
        packet_type: String,
        data: Vec<u8>
    }
}
//On error, returns how many bytes were missing
fn read_movements(data: &[u8]) -> Result<Vec<Position>,usize>
{
    let length = match data.get(0)
    {
        Some(l) => *l as usize,
        None => return Err(1),
    };
    
    if 1 + (length*4) != data.len()
    {
        return Err(1 + (length*4) - data.len());
    }

    //TODO there has got to be a better way to do this whole loop tbh
    let mut movements = Vec::with_capacity(length);
    for i in 0..length
    {
        movements.push(Position {
            x: i16::from_le_bytes(data[(1 + i*4)..(3 + i*4)].try_into().unwrap()),
            y: i16::from_le_bytes(data[(3 + i*4)..(5 + i*4)].try_into().unwrap())
        })
    }
    Ok(movements)
}

pub fn read_packet(stream: &mut TcpStream) -> Result<ClientPackets,ReadPacketErrors>
{
    let mut length_buff = [0; 4];
    stream.read_exact(&mut length_buff)?;
    let length = u32::from_le_bytes(length_buff);
    
    if length < MIN_PACKET_LENGTH as u32 {
        return Err(ReadPacketErrors::InvalidLengthError { length: length });
    }

    let mut type_buff = [0; 4];
    stream.read_exact(&mut type_buff)?;

    let mut data_buff = vec![0u8; (length-4) as usize];
    if (length - 4) > 0
    {
        stream.read_exact(&mut data_buff)?
    }

    let packet = match type_buff
    {
        PACKET_TYPE_PROTOCOL => if data_buff.len() == 2
            {
                ClientPackets::ProtocolRequest {
                    game_version: u16::from_le_bytes(data_buff[0..2].try_into().unwrap())
                }
            }
            else
            {
                return Err(ReadPacketErrors::UnexpectedDataAmount { got: data_buff.len(), expected: 2 })
            },  
        PACKET_TYPE_TEST => if data_buff.len() == CONNECTION_TEST_DATA_SIZE
            {
                ClientPackets::ConnectionTest { data: data_buff[0..CONNECTION_TEST_DATA_SIZE].try_into().unwrap() }
            }
            else
            {
                return Err(ReadPacketErrors::UnexpectedDataAmount { got: data_buff.len(), expected: CONNECTION_TEST_DATA_SIZE });
            },
        PACKET_TYPE_DEBUG_LOG => {
            if data_buff.len() < 4 {
                return Err(ReadPacketErrors::UnexpectedDataAmount { got: data_buff.len(), expected: 4 })
            }
            let strlen = u32::from_le_bytes(data_buff[0..4].try_into().unwrap());
            if data_buff.len() != 4 + strlen as usize {
                return Err(ReadPacketErrors::UnexpectedDataAmount { got: data_buff.len(), expected: 4 + strlen as usize })
            } 
            match std::str::from_utf8(&data_buff[4..])
            {
                Ok(msg) => ClientPackets::LogDebugMessage { message: msg.to_string() },
                Err(_) => return Err(ReadPacketErrors::InvalidDataError { packet_type: std::str::from_utf8(&PACKET_TYPE_DEBUG_LOG).unwrap().to_owned(), data: data_buff }),
            }
            },
        PACKET_TYPE_MAP_ATTRIBUTES => if data_buff.is_empty()
            {
                ClientPackets::MapAttributeRequest
            }
            else
            {
                return Err(ReadPacketErrors::UnexpectedDataAmount { got: data_buff.len(), expected: 0 })
            },
        PACKET_TYPE_ROOM => if data_buff.len() == 2
            {
                ClientPackets::RoomRequest { coords: RoomCoordinates { x: data_buff[0] as i8, y: data_buff[1] as i8 } }
            }
            else
            {
                return Err(ReadPacketErrors::UnexpectedDataAmount { got: data_buff.len(), expected: 2 })
            },
        PACKET_TYPE_CHANGE_COLOR =>
            {
                if data_buff.len() < 2
                {
                    return Err(ReadPacketErrors::UnexpectedDataAmount { got: data_buff.len(), expected: 2 })
                }
                let color = data_buff[0];
                match read_movements(&data_buff[1..])
                {
                    Ok(movements) => ClientPackets::ChangeColor { color: color, movements: movements },
                    Err(exp) => return Err(ReadPacketErrors::UnexpectedDataAmount { got: data_buff.len(), expected: data_buff.len() + exp })
                }
            },
        PACKET_TYPE_MY_POSITION => match read_movements(&data_buff)
            {
                Ok(movements) => ClientPackets::MyPosition { movements: movements },
                Err(exp) => return Err(ReadPacketErrors::UnexpectedDataAmount { got: data_buff.len(), expected: data_buff.len() + exp })
            },
        PACKET_TYPE_DRAW_ON_FIELD =>
            {
                if data_buff.len() < 6
                {
                    return Err(ReadPacketErrors::UnexpectedDataAmount { got: data_buff.len(), expected: 6 })
                }
                let pos = Position {
                    x: i16::from_le_bytes(data_buff[0..2].try_into().unwrap()),
                    y: i16::from_le_bytes(data_buff[2..4].try_into().unwrap())
                };
                let tile = data_buff[4];
                match read_movements(&data_buff[5..])
                {
                    Ok(movements) => ClientPackets::DrawOnField { position: pos, tile: tile, movements: movements },
                    Err(exp) => return Err(ReadPacketErrors::UnexpectedDataAmount { got: data_buff.len(), expected: data_buff.len() + exp })
                }
            },
        PACKET_TYPE_HIT_NON_PLAYER_UNIT =>
            {
                if data_buff.len() < 2
                {
                    return Err(ReadPacketErrors::UnexpectedDataAmount { got: data_buff.len(), expected: 2 })
                }
                let index = data_buff[0];
                match read_movements(&data_buff[1..])
                {
                    Ok(movements) => ClientPackets::HitNonPlayerUnit { index: index, movements: movements },
                    Err(exp) => return Err(ReadPacketErrors::UnexpectedDataAmount { got: data_buff.len(), expected: data_buff.len() + exp })
                }
            },
        PACKET_TYPE_MAKE_CORPSE => if data_buff.len() == 4
            {
                ClientPackets::MakeCorpse { position: Position {
                    x: i16::from_le_bytes(data_buff[0..2].try_into().unwrap()),
                    y: i16::from_le_bytes(data_buff[2..4].try_into().unwrap())
                } }
            }
            else
            {
                return Err(ReadPacketErrors::UnexpectedDataAmount { got: data_buff.len(), expected: 0 })
            },
        PACKET_TYPE_HEAVEN => match read_movements(&data_buff)
            {
                Ok(movements) => ClientPackets::Heaven { movements: movements },
                Err(exp) => return Err(ReadPacketErrors::UnexpectedDataAmount { got: data_buff.len(), expected: data_buff.len() + exp })
            },
        PACKET_TYPE_BYE => if data_buff.is_empty()
            {
                ClientPackets::Bye
            }
            else
            {
                return Err(ReadPacketErrors::UnexpectedDataAmount { got: data_buff.len(), expected: 0 })
            },
        _ => return Err(ReadPacketErrors::InvalidTypeError {
            chars: Vec::from(type_buff)
        })
        
    };

    return Ok(packet);
}


pub fn write_packet(stream: &mut TcpStream, packet: ServerPackets) -> Result<(), Error>
{
    match packet {
        ServerPackets::Welcome =>
            send_bodyless_packet(stream, &PACKET_TYPE_WELCOME),
        ServerPackets::Void =>
            send_bodyless_packet(stream, &PACKET_TYPE_VOID),
        ServerPackets::Protocol { protocol, version } => {
            let mut data = [0; PROTOCOL_BUFFER_SIZE + 2];
            data[..PROTOCOL_BUFFER_SIZE].copy_from_slice(&protocol);
            data[PROTOCOL_BUFFER_SIZE..].copy_from_slice(&version.to_le_bytes());
            send_body_packet(stream, &PACKET_TYPE_PROTOCOL, &data)
        },
        ServerPackets::ConnectionTest { data } =>
            send_body_packet(stream, &PACKET_TYPE_TEST, &data),
        ServerPackets::MapAttributesResponse { map_attributes } => 
            send_body_packet(stream, &PACKET_TYPE_MAP_ATTRIBUTES, &[
                &map_attributes.width.to_le_bytes(),
                &map_attributes.height.to_le_bytes(),
                &map_attributes.attributes[0..]].concat()),
        ServerPackets::RoomResponse { coords, room } => {
            let mut data = [0; 2 + (CLIENT_ROOM_WIDTH * CLIENT_ROOM_HEIGHT)];
            data[0] = coords.x as u8;
            data[1] = coords.y as u8;
            data[2..].copy_from_slice(&room.data);
            send_body_packet(stream, &PACKET_TYPE_ROOM, &data)
        },
        ServerPackets::Fields {
            client_state,
            client_color,
            client_items,
            weather,
            soaprunners,
            entities,
            tiles } => {
            if soaprunners.len() > CLIENT_MAX_PLAYERS {
                //Too many players!
                return Err(Error::from(ErrorKind::InvalidInput))
            }
            if entities.len() > CLIENT_MAX_ENTITIES {
                //Too many entities!
                return Err(Error::from(ErrorKind::InvalidInput))
            }
            if tiles.len() > u8::MAX as usize {
                //Too many tiles!
                return Err(Error::from(ErrorKind::InvalidInput))
            }
            let plen = 7
                + (soaprunners.len() * 6) + ((soaprunners.iter().map(|s| s.1.movements.len())).sum::<usize>() * 4)
                +    (entities.len() * 6) +    ((entities.iter().map(|e| e.1.movements.len())).sum::<usize>() * 4)
                + (tiles.len() * 4);
            let mut data = Vec::with_capacity(plen);
            data.push(client_state as u8);
            data.push(client_color as u8);
            data.push(client_items.bits());

            data.push(soaprunners.len() as u8);
            data.push(entities.len() as u8);
            data.push(tiles.len() as u8);
            data.push(weather as u8);

            for (i, s) in soaprunners {
                data.push(i as u8);
                data.push(s.teleport_trigger);
                data.push(s.sprite as u8);
                data.push(s.color as u8);
                data.push(s.items.bits());

                data.push(s.movements.len() as u8);
                for m in s.movements {
                    data.extend_from_slice(&m.x.to_le_bytes());
                    data.extend_from_slice(&m.y.to_le_bytes());
                }
            }
            for (i, e) in entities {
                data.push(i as u8);
                data.push(e.teleport_trigger);
                data.push(e.unit_state as u8);
                data.push(e.unit_type as u8);
                data.push(e.direction);

                data.push(e.movements.len() as u8);
                for m in e.movements {
                    data.extend_from_slice(&m.x.to_le_bytes());
                    data.extend_from_slice(&m.y.to_le_bytes());
                }
            }

            for ct in tiles {
                data.extend_from_slice(&ct.x.to_le_bytes());
                data.extend_from_slice(&ct.y.to_le_bytes());
                data.push(ct.tile);
                data.push(ct.padding);
            }

            return send_body_packet(stream, &PACKET_TYPE_FIELDS, &data)
        },
    }
}

fn send_bodyless_packet(stream: &mut TcpStream, packet_type: &[u8; 4]) -> Result<(), Error>
{
    let mut packet: [u8; 8] = [4,0,0,0,0,0,0,0];
    packet[4..].copy_from_slice(packet_type);
    return stream.write_all(&packet);
}

fn send_body_packet(stream: &mut TcpStream, packet_type: &[u8; 4], packet_data: &[u8]) -> Result<(), Error>
{
    let len = (packet_type.len() + packet_data.len()) as u32;
    return stream.write_all(&[&len.to_le_bytes(), packet_type, packet_data].concat());
}