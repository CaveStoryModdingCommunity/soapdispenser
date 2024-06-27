use std::path::PathBuf;
use std::{fs, process::exit};
use clap::{Parser, Subcommand};

mod legacy_map_conversion;
mod soaprun;
mod server;

use server::ServerConfig;
use server::SoaprunServer;

#[derive(Subcommand)]
#[clap(rename_all="PascalCase")]
enum Actions {
    ConvertRooms {
        in_dir: PathBuf,
        conversion_map_path: PathBuf,
        out_dir : Option<PathBuf>
    }
}

const CONFIG_PATH : &str = "config.json";

#[derive(Parser)]
struct Args
{
    #[command(subcommand)]
    action: Option<Actions>,
    #[arg(short = 'c', long = "config")]
    config: Option<PathBuf>
}

fn main()
{
    let args = Args::parse();

    let config_path = args.config.unwrap_or(PathBuf::from(CONFIG_PATH));

    let config_str = fs::read_to_string(&config_path).unwrap_or_else(|_error|
        {
            println!("Couldn't find config file at location {:?}. Is your working directory correct?", config_path);
            exit(1);
        });
    let config:ServerConfig = serde_json::from_str(&config_str).unwrap_or_else(|error| 
        {
            println!("Malformed config file: {:?}", error);
            exit(2);
        });

    if let Some(action) = args.action {
        match action {
            Actions::ConvertRooms { in_dir, conversion_map_path, out_dir } =>
            {
                let _ = legacy_map_conversion::convert_rooms(&in_dir, &conversion_map_path, &out_dir.unwrap_or(config.room_directory));
            },
        }
        exit(0);
    }
    
    println!("Starting server...");
    match SoaprunServer::new(&config) {
        Ok(server) => {
            match server.start_server(config.address) {
                Ok(()) => { }, //it worked, lol
                Err(e) => eprintln!("Error: {e}"),
            }
        },
        Err(e) => {
            eprintln!("Error: {e}");
        },
    };
    println!("Server closed!");
}
