use std::collections::HashMap;

use tokio::{
    sync::{
        mpsc::{Receiver, Sender, channel},
        oneshot,
    },
    task::JoinHandle,
};

use crate::error::ModuleManagementError;

#[derive(Debug, Clone)]
pub struct A3Module {
    pub uid: u32,
    pub id: u8,
    // name: String,
    // module_type: u8,
}

pub enum Operation {
    GetOrCreateIdByUid {
        uid: u32,
        resp: oneshot::Sender<Result<u8, ModuleManagementError>>,
    },
    Register {
        uid: u32,
        id: u8,
    },
    List {
        resp: oneshot::Sender<Result<Vec<A3Module>, ModuleManagementError>>,
    },
}

// TODO: Consider using sqlite
pub struct A3Modules {
    modules_by_id: HashMap<u8, A3Module>,
    modules_by_uid: HashMap<u32, A3Module>,
}

impl A3Modules {
    pub fn new() -> Self {
        Self {
            modules_by_uid: HashMap::new(),
            modules_by_id: HashMap::new(),
        }
    }

    pub fn get_or_create_id_by_uid(&mut self, uid: u32) -> u8 {
        let id = match self.modules_by_uid.get(&uid) {
            Some(module) => module.id,
            None => {
                let new_id = self.find_available_id();
                let module = A3Module { id: new_id, uid };
                self.modules_by_id.insert(new_id, module.clone());
                self.modules_by_uid.insert(uid, module);
                new_id
            }
        };
        return id;
    }

    pub fn register(&mut self, uid: u32, id: u8) {
        let module = A3Module { id, uid };
        self.modules_by_id.insert(module.id, module.clone());
        self.modules_by_uid.insert(module.uid, module);
    }

    pub fn list(&self) -> Vec<A3Module> {
        let mut modules_list = Vec::new();
        for (_, module) in &self.modules_by_id {
            modules_list.push(module.clone());
        }
        return modules_list;
    }

    //////////////////////////////////////////////////////////////////

    fn find_available_id(&self) -> u8 {
        for id in 1..=255 {
            if !self.modules_by_id.contains_key(&id) {
                return id;
            }
        }
        return 0;
    }
}

pub fn start() -> (Sender<Operation>, JoinHandle<()>) {
    let (operation_tx, operation_rx) = channel(8);
    let handle = tokio::spawn(async move {
        handle_requests(operation_rx).await;
    });
    return (operation_tx, handle);
}

async fn handle_requests(mut operation_rx: Receiver<Operation>) {
    let mut modules = A3Modules::new();
    loop {
        if let Some(request) = operation_rx.recv().await {
            match request {
                Operation::GetOrCreateIdByUid { uid, resp } => {
                    let id = modules.get_or_create_id_by_uid(uid);
                    resp.send(Ok(id)).unwrap();
                }
                Operation::Register { uid, id } => {
                    modules.register(uid, id);
                }
                Operation::List { resp } => {
                    let modules_list = modules.list();
                    resp.send(Ok(modules_list)).unwrap();
                }
            }
        }
    }
}
