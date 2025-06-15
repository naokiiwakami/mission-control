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

pub async fn request_name(can_tx: Sender<CanMessage>, id: u8, wire_id: u8) {
    let mut out_message = make_mission_control_message(a3::A3_MC_REQUEST_NAME, id);
    out_message.set_data(2, wire_id);
    out_message.set_data_length(3);
    can_tx.send(out_message).await.unwrap();
}

pub async fn continue_name(can_tx: Sender<CanMessage>, remote_id: u8) {
    send_op_and_id(can_tx, a3::A3_MC_CONTINUE_NAME, remote_id).await;
}

pub async fn request_config(can_tx: Sender<CanMessage>, id: u8, wire_id: u8) {
    let mut out_message = make_mission_control_message(a3::A3_MC_REQUEST_CONFIG, id);
    out_message.set_data(2, wire_id);
    out_message.set_data_length(3);
    can_tx.send(out_message).await.unwrap();
}

pub async fn continue_config(can_tx: Sender<CanMessage>, remote_id: u8) {
    send_op_and_id(can_tx, a3::A3_MC_CONTINUE_CONFIG, remote_id).await;
}

pub async fn request_uid_cancel(can_tx: Sender<CanMessage>, uid: u32) {
    let out_message = make_message_by_uid(uid, a3::A3_ADMIN_REQ_UID_CANCEL);
    can_tx.send(out_message).await.unwrap();
}

pub async fn im_sign_in(can_tx: Sender<CanMessage>, uid: u32) {
    let out_message = make_message_by_uid(uid, a3::A3_ADMIN_SIGN_IN);
    can_tx.send(out_message).await.unwrap();
}

pub async fn im_notify_id(can_tx: Sender<CanMessage>, uid: u32, id: u8) {
    let mut out_message = make_message_by_uid(uid, a3::A3_ADMIN_SIGN_IN);
    out_message.set_data(1, id);
    out_message.set_data_length(2);
    can_tx.send(out_message).await.unwrap();
}

async fn send_op_and_id(can_tx: Sender<CanMessage>, opcode: u8, remote_id: u8) {
    let out_message = make_mission_control_message(opcode, remote_id);
    can_tx.send(out_message).await.unwrap();
}

fn make_mission_control_message(opcode: u8, remote_id: u8) -> CanMessage {
    let mut out_message = CanMessage::new();
    out_message.set_id(a3::A3_ID_MISSION_CONTROL);
    out_message.set_extended(false);
    out_message.set_data_length(2);
    out_message.set_data(0, opcode);
    out_message.set_data(1, remote_id);
    return out_message;
}

fn make_message_by_uid(uid: u32, opcode: u8) -> CanMessage {
    let mut out_message = CanMessage::new();
    out_message.set_id(uid);
    out_message.set_extended(true);
    out_message.set_data_length(1);
    out_message.set_data(0, opcode);
    return out_message;
}
