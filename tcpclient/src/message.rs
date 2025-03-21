// 定义消息类型
#[derive(Debug)]
pub enum Message {
    Connect(String, u16),
    Disconnect,
    Send(String),
}