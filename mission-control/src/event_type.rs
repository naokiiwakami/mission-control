#[derive(Debug)]
pub enum EventType {
    NoEvent,
    MessageRx,
    MessageTx,
    UserConnection,
    RequestSent,
}
