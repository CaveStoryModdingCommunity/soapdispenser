use std::{
    io::{self, Read, Write},
    net::TcpStream,
};

use websocket::{sync::{server::IntoWs, Client}, OwnedMessage};
use crate::soaprun::packets::{MIN_PACKET_LENGTH, MAX_PACKET_LENGTH};

pub trait FramedStream {
    fn read_packet(&mut self) -> Result<Vec<u8>, io::Error>;
    fn write_packet(&mut self, packet: Vec<u8>) -> Result<(), io::Error>;
}

pub struct FramedTcpStream {
    stream: TcpStream,
}

impl FramedStream for FramedTcpStream {
    fn read_packet(&mut self) -> Result<Vec<u8>, io::Error> {
        let mut length_buff = [0; 4];
        self.stream.read_exact(&mut length_buff)?;
        let length = u32::from_le_bytes(length_buff);
        if (length as usize) < MIN_PACKET_LENGTH || MAX_PACKET_LENGTH < (length as usize) {
            return Err(io::Error::from(io::ErrorKind::OutOfMemory));
        }

        let mut data_buff = vec![0; length as usize];
        self.stream.read_exact(&mut data_buff)?;

        Ok(data_buff)
    }

    fn write_packet(&mut self, packet: Vec<u8>) -> Result<(), io::Error> {
        let len = packet.len() as u32;
        self.stream.write_all(&len.to_le_bytes())?;
        self.stream.write_all(&packet)?;

        Ok(())
    }
}

pub struct WebSocketStream {
    stream: websocket::sync::Client<TcpStream>,
}

impl FramedStream for WebSocketStream {
    fn read_packet(&mut self) -> Result<Vec<u8>, io::Error> {
        loop {
            match self
                .stream
                .recv_message()
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
            {
                OwnedMessage::Binary(data) => return Ok(data),
                OwnedMessage::Close(_) => {
                    return Err(io::Error::from(io::ErrorKind::ConnectionAborted))
                }
                _ => continue,
            }
        }
    }

    fn write_packet(&mut self, packet: Vec<u8>) -> Result<(), io::Error> {
        self.stream
            .send_message(&OwnedMessage::Binary(packet))
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        Ok(())
    }
}

fn accept_websocket(stream: TcpStream) -> Result<Client<TcpStream>, io::Error> {
    stream.set_nonblocking(false)?;
    let upgrade = stream.into_ws()
        .map_err(|_| io::Error::new(io::ErrorKind::Other, "Failed to upgrade to WebSocket"))?;
    let stream = upgrade.accept_with_limits(2*(4 + MAX_PACKET_LENGTH), 4 + MAX_PACKET_LENGTH)
        .map_err(|_| io::Error::new(io::ErrorKind::Other, "Failed to accept WebSocket"))?;

    Ok(stream)
}

pub fn probe_stream(stream: TcpStream) -> Result<Box<dyn FramedStream>, io::Error> {
    let mut buf = [0u8; 128];

    // Soaprun client only responds when a WLCM payload is sent.
    // HTTP clients will immediately send a GET request.
    // We give a small delay to allow the client to send a HTTP request. 
    // If we don't receive anything, we assume it's a Soaprun client.

    stream.set_nonblocking(true)?;
    std::thread::sleep(std::time::Duration::from_secs(1));

    if let Ok(size) = stream.peek(&mut buf) {
        if size > 0 && buf.starts_with(b"GET") {
            let stream = accept_websocket(stream)?;

            return Ok(Box::new(WebSocketStream { stream }));
        }
    }
    stream.set_nonblocking(false)?;

    Ok(Box::new(FramedTcpStream { stream }))
}