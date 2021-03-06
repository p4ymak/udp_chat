pub mod message;

use eframe::epi::RepaintSignal;
use log::{info, warn};
use message::{Command, Message};
use rusqlite::Connection;
use std::collections::HashSet;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

pub enum Recepients {
    One(Ipv4Addr),
    Peers,
    All,
}

pub struct UdpChat {
    socket: Option<Arc<UdpSocket>>,
    pub ip: Ipv4Addr,
    pub port: usize,
    pub name: String,
    sync_sender: mpsc::SyncSender<(Ipv4Addr, Message)>,
    sync_receiver: mpsc::Receiver<(Ipv4Addr, Message)>,
    pub message: Message,
    pub history: Vec<(Ipv4Addr, String)>,
    pub peers: HashSet<Ipv4Addr>,
    db: Option<Connection>,
    pub db_status: String,
}
impl UdpChat {
    pub fn new(name: String, port: usize, db_path: Option<PathBuf>) -> Self {
        let (tx, rx) = mpsc::sync_channel::<(Ipv4Addr, Message)>(0);
        let (db, db_status) = match db_path {
            Some(path) => (Connection::open(path).ok(), "DB: ready.".to_string()),
            None => (None, "DB! offline".to_string()),
        };
        warn!("{}", db_status);
        UdpChat {
            socket: None,
            ip: Ipv4Addr::UNSPECIFIED,
            port,
            name,
            sync_sender: tx,
            sync_receiver: rx,
            message: Message::empty(),
            history: Vec::<(Ipv4Addr, String)>::new(),
            peers: HashSet::<Ipv4Addr>::new(),
            db,
            db_status,
        }
    }

    pub fn prelude(&mut self, repaint_signal: Arc<dyn RepaintSignal>) {
        self.db_create();
        if let Ok(history) = self.db_get_all() {
            self.history = history;
        };
        self.connect();
        self.listen(repaint_signal);
        self.message = Message::enter(&self.name);
        self.send(Recepients::All);
    }

    fn connect(&mut self) {
        if let Some(local_ip) = local_ipaddress::get() {
            if let Ok(my_ip) = local_ip.parse::<Ipv4Addr>() {
                self.ip = my_ip;
                self.socket = match UdpSocket::bind(format!("{}:{}", self.ip, self.port)) {
                    Ok(socket) => {
                        socket.set_broadcast(true).unwrap();
                        socket.set_multicast_loop_v4(false).unwrap();
                        Some(Arc::new(socket))
                    }
                    _ => None,
                };
            }
        }
    }

    fn listen(&self, repaint_signal: Arc<dyn RepaintSignal>) {
        if let Some(socket) = &self.socket {
            let reader = Arc::clone(socket);
            let receiver = self.sync_sender.clone();
            let repaint_signal = Arc::clone(&repaint_signal);
            thread::spawn(move || {
                let mut buf = [0; 2048];
                let repaint_signal = Arc::clone(&repaint_signal);
                loop {
                    if let Ok((number_of_bytes, SocketAddr::V4(src_addr_v4))) =
                        reader.recv_from(&mut buf)
                    {
                        let ip = *src_addr_v4.ip();
                        if let Some(message) =
                            Message::from_be_bytes(&buf[..number_of_bytes.min(128)])
                        {
                            info!("{}: {}", ip, message);
                            repaint_signal.request_repaint();
                            receiver.send((ip, message)).ok();
                        }
                    }
                }
            });
        }
    }

    pub fn send(&mut self, mut addrs: Recepients) {
        match self.message.command {
            Command::Empty => return,
            Command::Text => {
                self.db_save(self.ip, &self.message.clone());
            }
            _ => (),
        }

        let bytes = self.message.to_be_bytes();
        if let Some(socket) = &self.socket {
            if self.peers.len() == 1 {
                addrs = Recepients::All;
            }
            let recepients: Vec<String> = match addrs {
                Recepients::All => (0..=254)
                    .map(|i| {
                        format!(
                            "{}.{}.{}.{}:{}",
                            self.ip.octets()[0],
                            self.ip.octets()[1],
                            self.ip.octets()[2],
                            i,
                            self.port
                        )
                    })
                    .collect(),
                Recepients::Peers => self
                    .peers
                    .iter()
                    .map(|ip| format!("{}:{}", ip, self.port))
                    .collect(),
                Recepients::One(ip) => vec![format!("{}:{}", ip, self.port)],
            };
            for recepient in recepients {
                socket.send_to(&bytes, recepient).ok();
            }
        }
        // self.message = Message::empty();
    }

    pub fn receive(&mut self) {
        if let Ok(message) = self.sync_receiver.try_recv() {
            match message.1.command {
                Command::Enter => {
                    info!("{} entered chat.", message.0);
                    if !self.peers.contains(&message.0) {
                        self.peers.insert(message.0);
                        if message.0 != self.ip {
                            self.message = Message::enter(&self.name);
                            self.send(Recepients::One(message.0));
                        }
                    }
                }
                Command::Text | Command::Repeat => {
                    if message.0 != self.ip {
                        self.db_save(message.0, &message.1);
                    }
                    let text = message.1.read_text();
                    self.history.push((message.0, text));
                    if !self.peers.contains(&message.0) {
                        self.peers.insert(message.0);
                        if message.0 != self.ip {
                            self.message = Message::enter(&self.name);
                            self.send(Recepients::One(message.0));
                        }
                    }
                }
                Command::Damaged => {
                    self.message =
                        Message::new(Command::AskToRepeat, message.1.id.to_be_bytes().to_vec());
                    self.send(Recepients::One(message.0));
                }
                Command::AskToRepeat => {
                    let id: u32 = u32::from_be_bytes(
                        (0..4)
                            .map(|i| *message.1.data.get(i).unwrap_or(&0))
                            .collect::<Vec<u8>>()
                            .try_into()
                            .unwrap(),
                    );
                    self.message = Message::retry_text(
                        id,
                        &self
                            .db_get_by_id(id)
                            .unwrap_or_else(|| String::from("NO SUCH MESSAGE! = (")),
                    );
                    self.send(Recepients::One(message.0));
                }
                Command::Exit => {
                    info!("{} left chat.", message.0);
                    self.peers.remove(&message.0);
                }
                _ => (),
            }
        }
    }

    fn db_create(&mut self) {
        if let Some(db) = &self.db {
            self.db_status = match db.execute(
                "create table if not exists chat_history (
                id integer primary key,
                ip text not null,
                message_text text not null
                )",
                [],
            ) {
                Ok(_) => "DB is ready.".to_string(),
                Err(err) => format!("DB Err: {}", err),
            };
            warn!("{}", self.db_status);
        }
    }
    fn db_save(&mut self, ip: Ipv4Addr, message: &Message) {
        if let Some(db) = &self.db {
            self.db_status = match db.execute(
                "INSERT INTO chat_history (id, ip, message_text) values (?1, ?2, ?3)",
                [message.id.to_string(), ip.to_string(), message.read_text()],
            ) {
                Ok(_) => "DB: appended.".to_string(),
                Err(err) => format!("DB! {}", err),
            };
            info!("{}", self.db_status);
        }
    }
    fn db_get_all(&mut self) -> rusqlite::Result<Vec<(Ipv4Addr, String)>> {
        if let Some(db) = &self.db {
            let mut stmt = db.prepare("SELECT ip, message_text FROM chat_history")?;
            let mut rows = stmt.query([])?;
            let mut story = Vec::<(String, String)>::new();
            while let Some(row) = rows.next()? {
                story.push((row.get(0)?, row.get(1)?));
            }

            Ok(story
                .iter()
                .map(|row| (row.0.parse::<Ipv4Addr>().unwrap(), row.1.to_owned()))
                .collect())
        } else {
            Ok(Vec::<(Ipv4Addr, String)>::new())
        }
    }
    fn db_get_by_id(&mut self, id: u32) -> Option<String> {
        if let Some(db) = &self.db {
            match db.query_row(
                "SELECT message_text FROM chat_history WHERE id = ?",
                [id],
                |row| row.get(0),
            ) {
                Ok(message_text) => message_text,
                Err(_) => None,
            }
        } else {
            None
        }
    }
    pub fn clear_history(&mut self) {
        if let Some(db) = &self.db {
            if let Some(db_path) = db.path() {
                self.db_status = match std::fs::remove_file(db_path) {
                    Ok(_) => "DB: Cleared".to_string(),
                    Err(err) => format!("DB! {}", err),
                };
                info!("{}", self.db_status);
                self.db_create();
            }
        }
        self.history = Vec::<(Ipv4Addr, String)>::new();
    }
}
