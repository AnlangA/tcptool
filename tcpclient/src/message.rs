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
        std::sync::Arc<std::sync::Mutex<Vec<String>>>,
        std::sync::Arc<std::sync::Mutex<Vec<(String, String)>>>,
    ), // (起始IP, 结束IP, 端口, 扫描结果, 扫描日志)
}
