use std::collections::HashMap;

use tokio::{
    sync::{
        mpsc::{Receiver, Sender, channel},
        oneshot,
    },
    task::JoinHandle,
};

use crate::error::{AppError, ErrorType};

#[derive(Debug, Clone)]
pub struct A3Module {
    pub uid: u32,
    pub id: u8,
    pub name: Option<String>,
    pub module_type: Option<String>,
    pub module_type_id: Option<u16>,
}

pub enum Operation {
    GetOrCreateIdByUid {
        uid: u32,
        resp: oneshot::Sender<Result<u8, AppError>>,
    },
    Register {
        uid: u32,
        id: u8,
    },
    Deregister {
        uid: u32,
    },
    List {
        resp: oneshot::Sender<Result<Vec<A3Module>, AppError>>,
    },
    GetById {
        id: u8,
        resp: oneshot::Sender<Result<A3Module, AppError>>,
    },
    SetProperties {
        id: u8,
        name: Option<String>,
        module_type: Option<String>,
        module_type_id: Option<u16>,
        // TODO: Return error when the module is not found
        // resp: oneshot::Sender<Result<(), AppError>>,
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
                let module = A3Module {
                    id: new_id,
                    uid,
                    name: Option::None,
                    module_type: Option::None,
                    module_type_id: Option::None,
                };
                self.modules_by_id.insert(new_id, module.clone());
                self.modules_by_uid.insert(uid, module);
                new_id
            }
        };
        return id;
    }

    pub fn register(&mut self, uid: u32, id: u8) {
        let module = A3Module {
            id,
            uid,
            name: Option::None,
            module_type: Option::None,
            module_type_id: Option::None,
        };
        self.modules_by_id.insert(module.id, module.clone());
        self.modules_by_uid.insert(module.uid, module);
    }

    pub fn deregister(&mut self, uid: u32) {
        if let Some(module) = self.modules_by_uid.remove(&uid) {
            self.modules_by_id.remove(&module.id);
        }
    }

    pub fn list(&self) -> Vec<A3Module> {
        let mut modules_list = Vec::new();
        for (_, module) in &self.modules_by_id {
            modules_list.push(module.clone());
        }
        modules_list
    }

    pub fn get_by_id(&self, id: u8) -> Result<A3Module, AppError> {
        match self.modules_by_id.get(&id) {
            Some(entry) => Ok(entry.clone()),
            None => Err(AppError::new(
                ErrorType::A3ModuleNotFound,
                format!("No such module ID: {}", id),
            )),
        }
    }

    pub fn set_properties(
        &mut self,
        id: u8,
        name: &Option<String>,
        module_type: &Option<String>,
        module_type_id: &Option<u16>,
    ) {
        if let Some(module) = self.modules_by_id.get_mut(&id) {
            module.name = name.clone().or(module.name.clone());
            module.module_type = module_type.clone().or(module.module_type.clone());
            module.module_type_id = module_type_id.clone().or(module.module_type_id.clone());
            if let Some(module2) = self.modules_by_uid.get_mut(&module.uid) {
                module2.name = name.clone().or(module2.name.clone());
                module2.module_type = module_type.clone().or(module2.module_type.clone());
                module2.module_type_id = module_type_id.clone().or(module2.module_type_id.clone());
            }
        }
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
                Operation::Deregister { uid } => {
                    modules.deregister(uid);
                }
                Operation::List { resp } => {
                    let modules_list = modules.list();
                    resp.send(Ok(modules_list)).unwrap();
                }
                Operation::GetById { id, resp } => {
                    resp.send(modules.get_by_id(id)).unwrap();
                }
                Operation::SetProperties {
                    id,
                    name,
                    module_type,
                    module_type_id,
                    // resp,
                } => {
                    modules.set_properties(id, &name, &module_type, &module_type_id);
                }
            }
        }
    }
}
