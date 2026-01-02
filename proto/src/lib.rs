pub mod protos {
    include!(concat!(env!("OUT_DIR"), "/protobuf_generated/generated.rs"));
}

use std::{
    io::{Read, Result, Write},
    net::TcpStream,
};

pub trait TcpWithSize {
    fn send(&mut self, msg: &[u8]) -> Result<()>;
    fn recieve(&mut self) -> Result<Vec<u8>>;
}

impl TcpWithSize for TcpStream {
    fn send(&mut self, msg: &[u8]) -> Result<()> {
        println!("Transmitting {} bytes", msg.len());
        self.write(&u32::try_from(msg.len()).unwrap().to_be_bytes())?;
        self.write_all(msg)?;
        Ok(())
    }

    fn recieve(&mut self) -> Result<Vec<u8>> {
        let mut message_length_buffer: [u8; 4] = [0; 4];
        self.read_exact(&mut message_length_buffer)?;
        let message_length: u32 = u32::from_be_bytes(message_length_buffer.try_into().unwrap());
        println!("Recieving {} bytes", message_length);
        let mut recieve_buffer = vec![0; message_length as usize];
        self.read_exact(&mut recieve_buffer)?;
        Ok(recieve_buffer)
    }
}
