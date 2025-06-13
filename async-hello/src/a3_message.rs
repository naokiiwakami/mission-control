use tokio::sync::mpsc::Sender;

use crate::{analog3 as a3, can_controller::CanMessage};

pub async fn sign_in(can_tx: Sender<CanMessage>) {
    let mut out_message = CanMessage::new();
    out_message.set_id(a3::A3_ID_MISSION_CONTROL);
    out_message.set_data_length(1);
    out_message.set_data(0, a3::A3_MC_SIGN_IN);
    can_tx.send(out_message).await.unwrap();
}

pub async fn assign_module_id(can_tx: Sender<CanMessage>, remote_uid: u32, remote_id: u8) {
    let mut out_message = CanMessage::new();
    out_message.set_id(a3::A3_ID_MISSION_CONTROL);
    out_message.set_data_length(6);
    out_message.set_data(0, a3::A3_MC_ASSIGN_MODULE_ID);
    out_message.set_data(1, ((remote_uid >> 24) & 0xff) as u8);
    out_message.set_data(2, ((remote_uid >> 16) & 0xff) as u8);
    out_message.set_data(3, ((remote_uid >> 8) & 0xff) as u8);
    out_message.set_data(4, (remote_uid & 0xff) as u8);
    out_message.set_data(5, remote_id);
    can_tx.send(out_message).await.unwrap();
}

pub async fn ping(can_tx: Sender<CanMessage>, remote_id: u8, enable_visual: bool) {
    let mut out_message = CanMessage::new();
    out_message.set_id(a3::A3_ID_MISSION_CONTROL);
    let length = if enable_visual { 3 } else { 2 };
    out_message.set_data_length(length);
    out_message.set_data(0, a3::A3_MC_PING);
    out_message.set_data(1, remote_id);
    if enable_visual {
        out_message.set_data(2, 1);
    }
    can_tx.send(out_message).await.unwrap();
}
