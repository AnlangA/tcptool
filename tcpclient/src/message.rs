// 定义消息类型
#[derive(Debug)]
pub enum Message {
    Connect(String, u16),
    Disconnect,
    Send(String),
    ScanIp(
        String,
        String,
        u16,
        u16,
        u64,
        std::sync::Arc<std::sync::Mutex<Vec<String>>>,
        std::sync::Arc<std::sync::Mutex<Vec<(String, String)>>>,
    ), // (起始IP, 结束IP, 起始端口, 结束端口, 超时时间, 扫描结果, 扫描日志)
}
