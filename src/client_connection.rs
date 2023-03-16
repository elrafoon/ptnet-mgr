use std::collections::HashMap;
use tokio::net::tcp::{ReadHalf, WriteHalf};
use tokio::sync::{oneshot, broadcast, Mutex};
use tokio::io::{AsyncWriteExt, AsyncReadExt};
use log::{warn, info, error};

use crate::ptnet::{ptnet_c, MAGIC_RESULT, MAGIC_SERVER_MESSAGE};

#[derive(Debug,Clone)]
pub struct Message {
    pub port: i32,
    pub header: ptnet_c::Header,
    pub payload: Vec<u8>
}

// Function that converts to byte array. (found on stackoverflow)
unsafe fn any_as_u8_slice<T: Sized>(p: &T) -> &[u8] {
    ::std::slice::from_raw_parts((p as *const T) as *const u8, ::std::mem::size_of::<T>())
}

unsafe fn any_as_u8_slice_mut<T: Sized>(p: &mut T) -> &mut [u8] {
    ::std::slice::from_raw_parts_mut((p as *mut T) as *mut u8, ::std::mem::size_of::<T>())
}


pub struct SharedState {
    id_gen: u16,
    request_map: HashMap<u16, oneshot::Sender<u16>>
}

pub struct ClientConnection {
    /// shared state lock
    pub lock: Mutex<SharedState>,
    /// used internally to broadcast server messages
    sender: broadcast::Sender<Message>,
    /// broadcasts server messages
    pub broadcast: broadcast::Receiver<Message>
}

impl ClientConnection {
    pub fn new() -> Self {
        let (sender, mut receiver) = broadcast::channel::<Message>(128);
        ClientConnection {
            lock: Mutex::new(SharedState { id_gen: 0, request_map: HashMap::new() }),
            sender: sender,
            broadcast: receiver

        }
    }
}

pub struct ClientConnectionSender<'a> {
    conn: &'a ClientConnection,
    guarded_writer: &'a Mutex<WriteHalf<'a>>
}

impl<'a> ClientConnectionSender<'a> {
    pub fn new(conn: &'a ClientConnection, guarded_writer: &'a Mutex<WriteHalf<'a>>) -> Self {
        ClientConnectionSender {
            conn: conn,
            guarded_writer: guarded_writer
        }
    }

    pub async fn send_message(&self, msg: &Message) -> Result<oneshot::Receiver<u16>, Box<dyn std::error::Error>> {
        let mut ss = self.conn.lock.lock().await;

        let raw_msg = ptnet_c::Message {
            id: ss.id_gen,
            iPort: msg.port,
            header: msg.header,
            payloadLength: msg.payload.len() as u8,
        };
        ss.id_gen += 1;

        let magic_slice: &[u8];
        let msg_slice: &[u8];

        unsafe {
            magic_slice = any_as_u8_slice(&ptnet_c::MAGIC_MESSAGE);
            msg_slice = any_as_u8_slice(&raw_msg);
        }

        let (sender, receiver) = oneshot::channel::<u16>();

        {
            let mut writer = self.guarded_writer.lock().await;

            writer.write_all(magic_slice).await?;
            writer.write_all(msg_slice).await?;
            writer.write_all(&msg.payload).await?;
        }

        ss.request_map.insert(raw_msg.id, sender);

        Ok(receiver)
    }
}

pub struct ClientConnectionDispatcher<'a> {
    conn: &'a ClientConnection,
    reader: &'a mut ReadHalf<'a>
}

impl<'a> ClientConnectionDispatcher<'a> {
    pub fn new(conn: &'a ClientConnection, reader: &'a mut ReadHalf<'a>) -> Self {
        ClientConnectionDispatcher {
            conn: conn,
            reader: reader
        }
    }

    pub async fn dispatch(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            let mut magic: ptnet_c::magic_t = 0;
            let mut magic_slice: &mut [u8];

            unsafe {
                magic_slice = any_as_u8_slice_mut(&mut magic);
            }

            self.reader.read_exact(&mut magic_slice).await?;
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
        let mut result = ptnet_c::MessageResult { msgId: 0, result: 0 };
        let mut result_slice: &mut [u8];

        unsafe {
            result_slice = any_as_u8_slice_mut(&mut result);
        }

        self.reader.read_exact(&mut result_slice).await?;

        {
            let mut ss = self.conn.lock.lock().await;

            match ss.request_map.remove(&result.msgId) {
                Some(sender) => sender.send(result.result).unwrap(),
                None => warn!("No request_map entry for msgId {}", result.msgId)
            };
        }

        Ok(())
    }

    async fn dispatch_server_message(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut raw_msg = ptnet_c::ServerMessage {
            iPort: 0,
            header: ptnet_c::Header { C: 0, address: [0; 6] },
            payloadLength: 0
        };
        let msg_slice: &mut [u8];

        unsafe {
            msg_slice = any_as_u8_slice_mut(&mut raw_msg);
        }

        self.reader.read_exact(msg_slice).await?;

        let mut pay: Vec<u8> = Vec::new();
        pay.resize(usize::from(raw_msg.payloadLength), 0);

        self.reader.read_exact(pay.as_mut_slice()).await?;

        let msg = Message {
            port: raw_msg.iPort as i32,
            header: raw_msg.header,
            payload: pay
        };

        self.conn.sender.send(msg).unwrap();

        Ok(())
    }
}
