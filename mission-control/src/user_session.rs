mod spec;

use std::cmp::max;

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    sync::mpsc::{Receiver, Sender, channel},
    sync::oneshot,
    task::JoinHandle,
};

use crate::{
    analog3 as a3,
    analog3::config::{Configuration, Property, Value},
    command::Command,
    error::{AppError, ErrorType},
    user_session::spec::Spec,
};

pub async fn start() -> std::io::Result<(Receiver<Command>, JoinHandle<()>)> {
    let (command_tx, command_rx) = channel(8);
    let listener = TcpListener::bind("127.0.0.1:9999").await?;
    let handle = tokio::spawn(async move {
        log::info!("Listening on port 9999");
        loop {
            // The second item contains the IP and port of the new connection.
            match listener.accept().await {
                Ok((stream, _)) => start_session(stream, command_tx.clone()),
                Err(e) => log::error!("User connection accept error: {:?}", e),
            }
        }
    });
    return Ok((command_rx, handle));
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

    pub async fn run(&mut self) -> std::io::Result<()> {
        self.stream
            .write_all(b"\r\n====================================\r\n welcome to analog3 mission control\r\n====================================\r\n\r\n")
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
                    let tokens: Vec<String> = Self::tokenize(&trimmed);
                    if tokens.is_empty() {
                        // do nothing
                        continue;
                    }
                    let command = tokens[0].trim();
                    match command {
                        "hello" => {
                            self.stream.write_all(b"hi\r\n").await?;
                        }
                        "hi" => self.hi().await?,
                        "list" => self.list().await?,
                        "ping" => self.ping(command, &tokens).await?,
                        "get-name" => self.get_name(&command, &tokens).await?,
                        "rename" => self.rename(&command, &tokens).await?,
                        "get-config" => self.get_config(&command, &tokens).await?,
                        "set-property" => self.set_property(&command, &tokens).await?,
                        "cancel-uid" => self.cancel_uid(&command, &tokens).await?,
                        "pretend-sign-in" => self.pretend_sign_in(&command, &tokens).await?,
                        "pretend-notify-id" => self.pretend_notify_id(&command, &tokens).await?,
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

    async fn hi(&mut self) -> std::io::Result<()> {
        let (resp_tx, resp_rx) = oneshot::channel();
        let command = Command::Hi { resp: resp_tx };
        self.command_tx.send(command).await.unwrap();
        return self.wait_and_handle_response(resp_rx, |r| r).await;
    }

    async fn list(&mut self) -> std::io::Result<()> {
        let (resp_tx, resp_rx) = oneshot::channel();
        let command = Command::List { resp: resp_tx };
        self.command_tx.send(command).await.unwrap();
        return self
            .wait_and_handle_response(resp_rx, |modules| {
                let reply = modules
                    .iter()
                    .map(|m| {
                        let module_type = match &m.module_type {
                            Some(value) => format!(" type={}", value),
                            None => "".to_string(),
                        };
                        let name = match &m.name {
                            Some(value) => format!(" name={}", value),
                            None => "".to_string(),
                        };
                        format!("uid={:08x} id={:02x}{}{}", m.uid, m.id, module_type, name)
                    })
                    .collect::<Vec<_>>()
                    .join("\r\n");
                return reply;
            })
            .await;
    }

    async fn ping(&mut self, command: &str, tokens: &Vec<String>) -> std::io::Result<()> {
        let specs = vec![Spec::u8("id", true), Spec::bool("visual", false)];
        let Some(params) = self.parse_params(command, tokens, &specs).await.unwrap() else {
            return Ok(());
        };

        let (resp_tx, resp_rx) = oneshot::channel();
        let id = params[0].as_u8().unwrap();
        let enable_visual = if params.len() > 1 {
            params[1].as_bool().unwrap()
        } else {
            false
        };
        let command = Command::Ping {
            id,
            enable_visual,
            resp: resp_tx,
        };
        self.command_tx.send(command).await.unwrap();
        self.stream
            .write_all(format!("ping to id {:02x} ... ", id).as_bytes())
            .await?;
        return self
            .wait_and_handle_response(resp_rx, |_| "ok".to_string())
            .await;
    }

    async fn get_name(&mut self, command: &str, tokens: &Vec<String>) -> std::io::Result<()> {
        let specs = vec![Spec::u8("id", true)];
        let Some(params) = self.parse_params(command, tokens, &specs).await.unwrap() else {
            return Ok(());
        };

        let (resp_tx, resp_rx) = oneshot::channel();
        let id = params[0].as_u8().unwrap();
        let command = Command::GetName { id, resp: resp_tx };
        self.command_tx.send(command).await.unwrap();

        return self.wait_and_handle_response(resp_rx, |name| name).await;
    }

    async fn rename(&mut self, command: &str, tokens: &Vec<String>) -> std::io::Result<()> {
        let specs = vec![Spec::u8("id", true), Spec::str("name", true)];
        let Some(params) = self.parse_params(command, tokens, &specs).await.unwrap() else {
            return Ok(());
        };

        let (resp_tx, resp_rx) = oneshot::channel();
        let id = params[0].as_u8().unwrap();
        let new_name = params[1].as_text().unwrap();
        let property = Property::text(a3::A3_PROP_NAME, &new_name);
        let command = Command::SetConfig {
            id,
            props: vec![property],
            resp: resp_tx,
        };
        self.command_tx.send(command).await.unwrap();

        return self
            .wait_and_handle_response(resp_rx, |_| "ok".to_string())
            .await;
    }

    async fn get_config(&mut self, command: &str, tokens: &Vec<String>) -> std::io::Result<()> {
        let specs = vec![Spec::u8("id", true)];
        let Some(params) = self.parse_params(command, tokens, &specs).await.unwrap() else {
            return Ok(());
        };

        let (resp_tx, resp_rx) = oneshot::channel();
        let id = params[0].as_u8().unwrap();
        let command = Command::GetConfig { id, resp: resp_tx };
        self.command_tx.send(command).await.unwrap();

        return self
            .wait_and_handle_response(resp_rx, |properties| {
                let config = Configuration::new(properties);
                // show only the common fields for now
                let mut key_values = Vec::<(String, String)>::new();
                let mut longest = 0;

                for i in 0..config.len() {
                    let name = config.prop_name(i);
                    let value = config.prop_value_as_string(i);
                    longest = max(name.len(), longest);
                    key_values.push((name, value));
                }

                let mut lines = Vec::<String>::new();
                lines.push("".to_string());
                for (name, value) in key_values {
                    let mut line: String = format!("  {}", name);
                    for _ in name.len()..longest {
                        line.push(' ');
                    }
                    line.push_str(" : ");
                    line.push_str(value.as_str());
                    lines.push(line);
                }
                lines.push("".to_string());
                return lines.join("\r\n");
            })
            .await;
    }

    async fn set_property(&mut self, command: &str, tokens: &Vec<String>) -> std::io::Result<()> {
        let specs = vec![
            Spec::u8("id", true),
            Spec::str("prop", true),
            Spec::str("name", true),
        ];
        let Some(params) = self.parse_params(command, tokens, &specs).await.unwrap() else {
            return Ok(());
        };

        let id = params[0].as_u8().unwrap();
        let property_name = params[1].as_text().unwrap();
        let property_value = params[2].as_text().unwrap();

        let (resp_tx, resp_rx) = oneshot::channel();

        if let Err(e) = self
            .set_property_core(id, &property_name, &property_value, resp_tx)
            .await
        {
            log::warn!("Operation failed: {:?}", e);
            self.stream
                .write_all(format!("Error: {:?}: {}\r\n", e.error_type, e.message).as_bytes())
                .await?;
            return Ok(());
        }

        return self
            .wait_and_handle_response(resp_rx, |_| "ok".to_string())
            .await;
    }

    async fn set_property_core(
        &mut self,
        id: u8,
        property_name: &String,
        property_value: &String,
        resp_tx: oneshot::Sender<Result<(), AppError>>,
    ) -> Result<(), AppError> {
        // Retrieve the schema of the module
        let (schema_resp_tx, schema_resp_rx) = oneshot::channel();
        let command = Command::GetSchema {
            id,
            resp: schema_resp_tx,
        };
        self.command_tx.send(command).await.unwrap();
        let schema = schema_resp_rx.await.unwrap()?;

        // Build the property
        let Some(property_def) = schema.get_property_def_by_name(&property_name) else {
            return Err(AppError::new(
                ErrorType::UserCommandInvalidRequest,
                format!("No such property: {}", property_name),
            ));
        };
        let property =
            Property::from_string(property_def.id, &property_value, &property_def.value_type)?;

        // Send a setconfig request
        let command2 = Command::SetConfig {
            id,
            props: vec![property],
            resp: resp_tx,
        };
        self.command_tx.send(command2).await.unwrap();
        return Ok(());
    }

    async fn cancel_uid(&mut self, command: &str, tokens: &Vec<String>) -> std::io::Result<()> {
        let specs = vec![Spec::u32("uid", true)];
        let Some(params) = self.parse_params(command, tokens, &specs).await.unwrap() else {
            return Ok(());
        };

        let (resp_tx, resp_rx) = oneshot::channel();
        let uid = params[0].as_u32().unwrap();
        let command = Command::RequestUidCancel { uid, resp: resp_tx };
        self.command_tx.send(command).await.unwrap();
        self.stream
            .write_all(format!("request UID cancellation: {:08x} ... ", uid).as_bytes())
            .await?;
        return self
            .wait_and_handle_response(resp_rx, |_| "sent".to_string())
            .await;
    }

    async fn pretend_sign_in(
        &mut self,
        command: &str,
        tokens: &Vec<String>,
    ) -> std::io::Result<()> {
        let specs = vec![Spec::u32("uid", true)];
        let Some(params) = self.parse_params(command, tokens, &specs).await.unwrap() else {
            return Ok(());
        };

        let (resp_tx, resp_rx) = oneshot::channel();
        let uid = params[0].as_u32().unwrap();
        let command = Command::PretendSignIn { uid, resp: resp_tx };
        self.command_tx.send(command).await.unwrap();
        self.stream
            .write_all(format!("pseudo sign-in with UID {:08x} ... ", uid).as_bytes())
            .await?;
        return self
            .wait_and_handle_response(resp_rx, |_| "sent".to_string())
            .await;
    }

    async fn pretend_notify_id(
        &mut self,
        command: &str,
        tokens: &Vec<String>,
    ) -> std::io::Result<()> {
        let specs = vec![Spec::u32("uid", true), Spec::u8("id", true)];
        let Some(params) = self.parse_params(command, tokens, &specs).await.unwrap() else {
            return Ok(());
        };

        let (resp_tx, resp_rx) = oneshot::channel();
        let uid = params[0].as_u32().unwrap();
        let id = params[1].as_u8().unwrap();
        let command = Command::PretendNotifyId {
            uid,
            id,
            resp: resp_tx,
        };
        self.command_tx.send(command).await.unwrap();
        self.stream
            .write_all(
                format!("pseudo notify-id with UID {:08x} ID {:02x} ... ", uid, id).as_bytes(),
            )
            .await?;
        return self
            .wait_and_handle_response(resp_rx, |_| "sent".to_string())
            .await;
    }

    // Utilities ////////////////////////////////////////////////////////////////

    fn tokenize(input: &str) -> Vec<String> {
        let mut tokens = Vec::new();
        let mut current = String::new();
        let mut chars = input.chars().peekable();
        let mut in_quotes: Option<char> = None;

        while let Some(c) = chars.next() {
            match c {
                // entering or exiting quotes
                '\'' | '"' => {
                    if in_quotes == Some(c) {
                        // closing matching quote
                        in_quotes = None;
                    } else if in_quotes.is_none() {
                        // starting new quoted section
                        in_quotes = Some(c);
                    } else {
                        // different quote inside quotes -> treat as normal char
                        current.push(c);
                    }
                }

                // whitespace: token delimiter only when NOT in quotes
                c if c.is_whitespace() && in_quotes.is_none() => {
                    if !current.is_empty() {
                        tokens.push(current.clone());
                        current.clear();
                    }
                }

                // normal character
                _ => current.push(c),
            }
        }

        // push final token if exists
        if !current.is_empty() {
            tokens.push(current);
        }

        tokens
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

    async fn wait_and_handle_response<T, F>(
        &mut self,
        resp_rx: oneshot::Receiver<Result<T, AppError>>,
        stringify: F,
    ) -> std::io::Result<()>
    where
        F: Fn(T) -> String,
    {
        match resp_rx.await.unwrap() {
            Ok(response) => {
                let reply = stringify(response);
                self.stream
                    .write_all(format!("{}\r\n", reply).as_bytes())
                    .await?;
            }
            Err(e) => {
                log::warn!("Operation failed: {:?}", e);
                let error_message = match e.error_type {
                    ErrorType::Timeout => "timeout\r\n".to_string(),
                    _ => format!("Error: {:?}: {}\r\n", e.error_type, e.message),
                };
                self.stream.write_all(error_message.as_bytes()).await?;
            }
        }
        return Ok(());
    }
}
