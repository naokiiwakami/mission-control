mod spec;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{Receiver, Sender, channel};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::analog3::Value;
use crate::operation::{Command, OperationResult, Request};
use crate::user_session::spec::Spec;

pub fn start() -> (Receiver<Command>, JoinHandle<()>) {
    let (command_tx, command_rx) = channel(8);
    let handle = tokio::spawn(async move {
        let listener = TcpListener::bind("127.0.0.1:9999").await.unwrap(); // TODO: Handle error more gracefully
        log::info!("Listening on port 9999");
        loop {
            // The second item contains the IP and port of the new connection.
            let (stream, _) = listener.accept().await.unwrap();
            start_session(stream, command_tx.clone());
        }
    });
    return (command_rx, handle);
}

fn start_session(stream: TcpStream, command_tx: Sender<Command>) {
    tokio::spawn(async move {
        let mut session = Session::new(stream, command_tx);
        session.run().await.unwrap();
    });
}

struct Session {
    stream: BufReader<TcpStream>,
    command_tx: Sender<Command>,
}

impl Session {
    pub fn new(stream: TcpStream, command_tx: Sender<Command>) -> Self {
        Self {
            stream: BufReader::new(stream),
            command_tx,
        }
    }

    async fn list(&mut self) -> std::io::Result<()> {
        let (tx, rx) = oneshot::channel();
        let command = Command::List { resp: tx };
        self.command_tx.send(command).await.unwrap();
        match rx.await.unwrap() {
            Ok(response) => {
                let reply = response
                    .iter()
                    .map(|m| format!("uid {:08x} id {:02x}", m.uid, m.id))
                    .collect::<Vec<_>>()
                    .join("\r\n");
                self.stream
                    .write_all(format!("{}\r\n", reply).as_bytes())
                    .await?;
            }
            Err(e) => {
                log::warn!("Operation failed: {:?}", e);
                self.stream.write_all(b"error\r\n").await?;
            }
        }
        return Ok(());
    }

    async fn ping(&mut self, command: &str, tokens: &Vec<String>) -> std::io::Result<()> {
        let specs = vec![Spec::u8("id", true), Spec::bool("visual", false)];
        let Some(params) = self.parse_params(command, tokens, &specs).await.unwrap() else {
            return Ok(());
        };

        let (tx, rx) = oneshot::channel();
        let id = params[0].as_u8().unwrap();
        let enable_visual = if params.len() > 1 {
            params[1].as_bool().unwrap()
        } else {
            false
        };
        let command = Command::Ping {
            id,
            enable_visual,
            resp: tx,
        };
        self.command_tx.send(command).await.unwrap();
        self.stream
            .write_all(format!("ping to id {:02x} ...", id).as_bytes())
            .await?;
        match rx.await.unwrap() {
            Ok(()) => {
                self.stream.write_all(" ok\r\n".as_bytes()).await?;
            }
            Err(e) => {
                log::warn!("Operation failed: {:?}", e);
                self.stream.write_all(b"error\r\n").await?;
            }
        }
        return Ok(());
    }

    async fn parse_params(
        &mut self,
        command: &str,
        tokens: &Vec<String>,
        specs: &Vec<Spec>,
    ) -> std::io::Result<Option<Vec<Value>>> {
        let mut params = Vec::new();
        for (i, spec) in specs.iter().enumerate() {
            if tokens.len() <= i + 1 {
                if spec.required {
                    self.usage(command, specs).await?;
                    return Ok(None);
                }
                break;
            }
            if let Ok(param) = (spec.parse)(&tokens[i + 1]) {
                params.push(param);
            } else {
                self.stream
                    .write_all(format!("Invalid {}\r\n", spec.name).as_bytes())
                    .await?;
                return Ok(None);
            }
        }
        return Ok(Some(params));
    }

    async fn usage(&mut self, command: &str, specs: &Vec<Spec>) -> std::io::Result<()> {
        let mut out = String::new();
        out += format!("Usage {}", command).as_str();
        for spec in specs {
            if spec.required {
                out += format!(" <{}>", spec.name).as_str();
            } else {
                out += format!(" [{}]", spec.name).as_str();
            }
        }
        out += "\r\n";
        self.stream.write_all(out.as_bytes()).await?;
        return Ok(());
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
                        "hi" => {
                            let (tx, rx) = oneshot::channel();
                            let command = Command::Hi { resp: tx };
                            self.command_tx.send(command).await.unwrap();
                            match rx.await {
                                Ok(response) => {
                                    self.stream.write_all(response.as_bytes()).await?;
                                }
                                Err(e) => {
                                    log::warn!("Operation failed: {:?}", e);
                                    self.stream.write_all(b"error\r\n").await?;
                                }
                            }
                        }
                        "list" => self.list().await?,
                        "ping" => self.ping(command, &tokens).await?,
                        /*
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
