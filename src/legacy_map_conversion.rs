use std::{fs::{self, File}, path::{Path, PathBuf}};
use std::io::{self, BufRead};
use glob::glob;
use encoding_rs::SHIFT_JIS;
use encoding_rs_io::DecodeReaderBytesBuilder;
use constcat::concat;
use crate::server::{ROOM_EXTENSION, ROOM_COORD_SEPARATOR};
use crate::soaprun::rooms::{CLIENT_ROOM_HEIGHT, CLIENT_ROOM_WIDTH};

#[derive(Debug)]
pub enum LegacyRoomReadErrors
{
    FileReadError {
        inner: io::Error
    },
    NotEnoughLinesError {
        got: usize,
        expected: usize,
    },
    LineDecodeError {
        line: usize,
        inner: io::Error
    },
    InvalidLineLengthError {
        line: usize,
        got: usize,
        expected: usize
    },
    InvalidCharacterError {
        line: usize,
        character: usize
    },
    CharacterOutOfRange {
        line: usize,
        character: usize,
        got: u8,
        max: u8,
    }
}

impl std::error::Error for LegacyRoomReadErrors {}

impl std::fmt::Display for LegacyRoomReadErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let _ = write!(f, "Error: ");
        let _ = match self
        {
            LegacyRoomReadErrors::FileReadError { inner } => 
                writeln!(f, "An IO error occured while opening the file: {:?}", inner),
            LegacyRoomReadErrors::NotEnoughLinesError { got, expected } =>
                writeln!(f, "Only got {got}/{expected} lines"),
            LegacyRoomReadErrors::LineDecodeError { line, inner } =>
                writeln!(f, "An IO error occurred while parsing line {line}: {:?}", inner),
            LegacyRoomReadErrors::InvalidLineLengthError { line, got, expected } =>
                writeln!(f, "Expected line {line} to be {expected} characters long, but it was {got}"),
            LegacyRoomReadErrors::InvalidCharacterError { line, character } =>
                writeln!(f, "Encountered invalid character on line {line} at position {character}"),
            LegacyRoomReadErrors::CharacterOutOfRange { line, character, got, max } => 
                writeln!(f, "The tile on line {line} at position {character} was not in the provided conversion map (got {got}, but the conversion map only goes up to {max})"),
        };
        return Ok(());
    }
}

const LEGACY_ROOM_MAX_LINES: usize = CLIENT_ROOM_HEIGHT + 1;
pub fn read_legacy_room(path: &PathBuf, conversion_map: &Vec<u8>, include_name: Option<bool>) -> Result<(String, Vec<u8>), LegacyRoomReadErrors>
{
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) => return Err(LegacyRoomReadErrors::FileReadError { inner: e }),
    };
    let mut iter = io::BufReader::new(
        DecodeReaderBytesBuilder::new()
        .encoding(Some(SHIFT_JIS))
        .build(file)).lines();
    let mut lines = Vec::with_capacity(LEGACY_ROOM_MAX_LINES);

    //try to read all room lines, plus the name at the top
    for i in 0..LEGACY_ROOM_MAX_LINES {
        match iter.next() {
            Some(Ok(line)) => lines.push(line),
            Some(Err(err)) => return Err(LegacyRoomReadErrors::LineDecodeError { line: i, inner: err }),
            None => {
                match include_name {
                    //missing a line while expecting a map name is an instant fail
                    Some(true) => {
                        return Err(LegacyRoomReadErrors::NotEnoughLinesError { got: lines.len(), expected: LEGACY_ROOM_MAX_LINES })
                    },
                    //missing a line when there is NO map name is fine as long as we have enough data
                    //if we weren't told what to expect for the map name, missing a line forces us to the no name case
                    Some(false) | None => {
                        if lines.len() < CLIENT_ROOM_HEIGHT {
                            return Err(LegacyRoomReadErrors::NotEnoughLinesError { got: lines.len(), expected: CLIENT_ROOM_HEIGHT })
                        } else {
                            break
                        }
                    },
                }
            },
        };
    }
    //map name was introduced in v0030
    let (map_name, start) = match lines.len() {
        LEGACY_ROOM_MAX_LINES => (lines[0].to_owned(), 1),
        CLIENT_ROOM_HEIGHT => (String::new(), 0),
        _ => unreachable!()
    };

    let mut data = Vec::with_capacity(CLIENT_ROOM_WIDTH*CLIENT_ROOM_HEIGHT);
    for (i, line) in lines.iter().skip(start).enumerate() {
        if line.len() != CLIENT_ROOM_WIDTH {
            return Err(LegacyRoomReadErrors::InvalidLineLengthError { line: i, got: line.len(), expected: CLIENT_ROOM_WIDTH })
        }
        for c in line.chars().enumerate() {
            data.push(match c.1.to_digit(10) {
                Some(val) => match conversion_map.get(val as usize) {
                    Some(t) => *t,
                    None => return Err(LegacyRoomReadErrors::CharacterOutOfRange { line: i, character: c.0, got: val as u8, max: conversion_map.len() as u8 }),
                },
                None => return Err(LegacyRoomReadErrors::InvalidCharacterError { line: i, character: c.0 })
            });
        }
    }
    assert_eq!(data.len(), CLIENT_ROOM_WIDTH*CLIENT_ROOM_HEIGHT);

    return Ok((map_name, data));
}

const LEGACY_ROOM_EXTENSION : &str = "dat";

pub fn convert_rooms(in_dir: &Path, conversion_map_path: &Path, out_dir: &Path) -> Result<(),io::Error>
{
    if !out_dir.exists() {
        fs::create_dir(out_dir)?;
    }

    let conversion_map = fs::read(conversion_map_path)?;
    if conversion_map.len() > u8::MAX as usize {
        return Err(io::Error::from(io::ErrorKind::InvalidData))
    }

    let buff = Path::new(in_dir).join(concat!("*.", LEGACY_ROOM_EXTENSION));
    let mut names = Vec::new();
    for room in glob(buff.to_str().unwrap()).unwrap().filter_map(Result::ok)
    {
        let fname = room.file_stem().unwrap().to_str().unwrap();
        let out_name = Vec::from_iter(fname.split('-'));
        if out_name.len() != 3 {
            println!("Failed to split \"{fname}\" into 3 sections");
            continue;
        }
        if out_name[0] != "map" {
            println!("First word of \"{fname}\" wasn't \"map\"");
            continue;
        }
        let y = out_name[1].parse::<u8>();
        let x = out_name[2].parse::<u8>();
        if x.is_err() || y.is_err() {
            println!("Unable to parse ({}, {}) as a valid room coordinate", out_name[1], out_name[2]);
            continue;
        }
        let out_name = format!("{}{}{}.{}", x.unwrap(), ROOM_COORD_SEPARATOR, y.unwrap(), ROOM_EXTENSION);

        match read_legacy_room(&room, &conversion_map, None)
        {
            Ok(result) => {
                println!("Processed {:?}: {:?}", &room, &result.0);
                let out_path = out_dir.join(out_name);
                match fs::write(&out_path, result.1)
                {
                    Ok(()) => names.push(
                        room.to_str().unwrap().to_owned() + " -> "
                        + out_path.to_str().unwrap() + " (" + &result.0 + ")"
                    ),
                    Err(err) => println!("Error while saving! {:?}", err)
                };
            },
            Err(err) => println!("Failed to convert {:?}! {:?}", room, err),
        }
    }
    fs::write(out_dir.join("names.txt"), names.join("\n") )?;
    return Ok(());
}