use std::collections::HashMap;
use tokio::sync::{oneshot, broadcast, Mutex};
use tokio::net::{TcpStream};
use tokio::io::{AsyncWriteExt, AsyncReadExt};
use log::{warn, info, error};

use crate::ptlink::connection::{MAGIC_RESULT, MAGIC_SERVER_MESSAGE};
use crate::{ptlink, ptlink::connection::magic_t };

#[derive(Debug,Clone)]
pub struct Message {
    port: i32,
    header: ptlink::connection::Header,
    payload: Vec<u8>
}

// Function that converts to byte array. (found on stackoverflow)
unsafe fn any_as_u8_slice<T: Sized>(p: &T) -> &[u8] {
    ::std::slice::from_raw_parts((p as *const T) as *const u8, ::std::mem::size_of::<T>())
}

unsafe fn any_as_u8_slice_mut<T: Sized>(p: &mut T) -> &mut [u8] {
    ::std::slice::from_raw_parts_mut((p as *mut T) as *mut u8, ::std::mem::size_of::<T>())
}


struct SharedState {
    id_gen: u16,
    request_map: HashMap<u16, oneshot::Sender<u16>>
}

pub struct ClientConnection<'a> {
    stream: &'a mut TcpStream,
    lock: Mutex<SharedState>,
    pub broadcast: broadcast::Receiver<Message>,
    sender: broadcast::Sender<Message>
}

impl<'a> ClientConnection<'a> {
    pub fn new(stream: &'a mut TcpStream) -> Self {
        let (sender, mut receiver) = broadcast::channel::<Message>(128);
        ClientConnection {
            stream: stream,
            lock: Mutex::new(SharedState { id_gen: 0, request_map: HashMap::new() }),
            broadcast: receiver,
            sender: sender
        }
    }

    pub async fn send_message(&mut self, msg: &Message) -> Result<oneshot::Receiver<u16>, Box<dyn std::error::Error>> {
        let raw_msg = ptlink::connection::Message {
            id: 0,
            iPort: msg.port,
            header: msg.header,
            payloadLength: msg.payload.len() as u8,
        };
        let magic_slice: &[u8];
        let msg_slice: &[u8];

        unsafe {
            magic_slice = any_as_u8_slice(&ptlink::connection::MAGIC_MESSAGE);
            msg_slice = any_as_u8_slice(&raw_msg);
        }

        let (sender, receiver) = oneshot::channel::<u16>();
        {
            let mut ss = self.lock.lock().await;

            self.stream.write_all(magic_slice).await?;
            self.stream.write_all(msg_slice).await?;
            self.stream.write_all(&msg.payload).await?;

            let id = ss.id_gen;
            ss.id_gen += 1;

            ss.request_map.insert(id, sender);
        }

        Ok(receiver)
    }

    pub async fn dispatch(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            let mut magic: magic_t = 0;
            let mut magic_slice: &mut [u8];

            unsafe {
                magic_slice = any_as_u8_slice_mut(&mut magic);
            }

            self.stream.read_exact(&mut magic_slice).await?;
            match magic {
                MAGIC_RESULT => self.dispatch_result().await,
                MAGIC_SERVER_MESSAGE => self.dispatch_server_message().await,
                x => Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Unsupported magic {:#04x}", x)
                ).into())
            }?;
        }
    }

    async fn dispatch_result(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut result = ptlink::connection::MessageResult { msgId: 0, result: 0 };
        let mut result_slice: &mut [u8];

        unsafe {
            result_slice = any_as_u8_slice_mut(&mut result);
        }

        self.stream.read_exact(&mut result_slice).await?;

        {
            let mut ss = self.lock.lock().await;

            match ss.request_map.remove(&result.msgId) {
                Some(sender) => sender.send(result.result).unwrap(),
                None => warn!("No request_map entry for msgId {}", result.msgId)
            };
        }

        Ok(())
    }

    async fn dispatch_server_message(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut raw_msg = ptlink::connection::ServerMessage {
            iPort: 0,
            header: ptlink::connection::Header { C: 0, address: [0; 6] },
            payloadLength: 0
        };
        let msg_slice: &mut [u8];

        unsafe {
            msg_slice = any_as_u8_slice_mut(&mut raw_msg);
        }

        self.stream.read_exact(msg_slice).await?;

        let mut pay: Vec<u8> = Vec::new();
        pay.resize(usize::from(raw_msg.payloadLength), 0);

        self.stream.read_exact(pay.as_mut_slice()).await?;

        let msg = Message {
            port: raw_msg.iPort as i32,
            header: raw_msg.header,
            payload: pay
        };

        self.sender.send(msg).unwrap();

        Ok(())
    }
}
