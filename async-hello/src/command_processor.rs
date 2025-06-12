use crate::operation::{OperationResult, Request};
use dashmap::DashMap;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{Receiver, Sender, channel};
use tokio::task::JoinHandle;

pub fn start(
    sessions: Arc<DashMap<u32, Sender<OperationResult>>>,
) -> (Receiver<Request>, JoinHandle<()>) {
    let (command_tx, command_rx) = channel(8);
    let handle = tokio::spawn(async move {
        let listener = TcpListener::bind("127.0.0.1:9999").await.unwrap(); // TODO: Handle error more gracefully
        log::info!("Listening on port 9999");
        let mut next_session_id = 1u32;
        loop {
            // The second item contains the IP and port of the new connection.
            let (stream, _) = listener.accept().await.unwrap();
            let session_id = next_session_id;
            next_session_id += 1;
            start_session(stream, session_id, sessions.clone(), command_tx.clone());
        }
    });
    return (command_rx, handle);
}

fn start_session(
    stream: TcpStream,
    session_id: u32,
    sessions: Arc<DashMap<u32, Sender<OperationResult>>>,
    command_tx: Sender<Request>,
) {
    tokio::spawn(async move {
        let (result_tx, result_rx) = channel(8);
        sessions.insert(session_id, result_tx);
        let mut session = Session::new(session_id, stream, command_tx, result_rx);
        session.run().await;
        sessions.remove(&session_id);
    });
}

struct Session {
    session_id: u32,
    stream: BufReader<TcpStream>,
    command_tx: Sender<Request>,
    result_rx: Receiver<OperationResult>,
}

impl Session {
    pub fn new(
        session_id: u32,
        stream: TcpStream,
        command_tx: Sender<Request>,
        result_rx: Receiver<OperationResult>,
    ) -> Self {
        Self {
            session_id,
            stream: BufReader::new(stream),
            command_tx,
            result_rx,
        }
    }

    pub async fn run(&mut self) -> std::io::Result<()> {
        self.stream
            .write_all(b"welcome to analog3 mission control\r\n")
            .await?;

        loop {
            self.stream.write_all(b"analog3> ").await?;
            let mut line = String::new();
            match self.stream.read_line(&mut line).await? {
                0 => {
                    log::debug!("Connection closed");
                    return Ok(());
                }
                _ => {
                    let trimmed = line.trim().to_string();
                    log::debug!("Received: {}", trimmed);
                    let tokens: Vec<String> = trimmed.split(" ").map(str::to_string).collect();
                    if tokens.is_empty() {
                        // do nothing
                        continue;
                    }
                    let command = tokens[0].trim();
                    match command {
                        "hello" => {
                            self.stream.write_all(b"hi\r\n").await?;
                        }
                        /*
                        "list" => self.process(&command, Operation::List, &tokens, &vec![])?,
                        "ping" => self.process(
                            &command,
                            Operation::Ping,
                            &tokens,
                            &vec![Spec::u8("id", true), Spec::bool("visual", false)],
                        )?,
                        "get-name" => self.get_name(&command, &tokens)?,
                        "get-config" => self.get_config(&command, &tokens)?,
                        "cancel-uid" => self.process(
                            &command,
                            Operation::RequestUidCancel,
                            &tokens,
                            &vec![Spec::u32("uid", true)],
                        )?,
                        "pretend-sign-in" => self.process(
                            &command,
                            Operation::PretendSignIn,
                            &tokens,
                            &vec![Spec::u32("uid", true)],
                        )?,
                        "pretend-notify-id" => self.process(
                            &command,
                            Operation::PretendNotifyId,
                            &tokens,
                            &vec![Spec::u32("uid", true), Spec::u8("id", true)],
                        )?,
                        */
                        "quit" => {
                            self.stream.write_all(b"bye!\r\n").await?;
                            return Ok(());
                        }
                        "" => {
                            // do nothing
                        }
                        _ => {
                            self.stream
                                .write_all(format!("{}: Unknown command\r\n", command).as_bytes())
                                .await?;
                        }
                    }
                }
            }
        }
    }
}
