use crate::utils::get_timestamp;
use std::sync::{Arc, Mutex};
use tokio::io::AsyncReadExt;

// 改进的异步处理数据接收的函数
pub async fn handle_data_reception(
    messages: Arc<Mutex<Vec<(String, String)>>>,
    mut port: tokio::net::tcp::OwnedReadHalf
) {
    messages.lock().unwrap().push((get_timestamp(), "数据接收通道已建立".to_string()));
    
    let mut read_buffer = [0u8; 1024];
    
    // 持续从读取半部分读取数据，直到连接关闭或发生错误
    loop {
        // 从读取半部分读取数据
        match port.read(&mut read_buffer).await {
            Ok(0) => {
                messages.lock().unwrap().push((get_timestamp(), "服务器关闭了连接".to_string()));
                break;
            }
            Ok(n) => {
                // 处理收到的数据
                match String::from_utf8(read_buffer[..n].to_vec()) {
                    Ok(data) => {
                        messages.lock().unwrap().push((get_timestamp(), format!("收到: {}", data)));
                    }
                    Err(_) => {
                        // 处理非UTF-8数据
                        let hex_data: Vec<String> = read_buffer[..n]
                            .iter()
                            .map(|b| format!("{:02X}", b))
                            .collect();
                        messages.lock().unwrap().push((get_timestamp(), format!("收到二进制数据: {}", hex_data.join(" "))));
                    }
                }
            }
            Err(e) => {
                // 详细分类错误类型
                let error_msg = match e.kind() {
                    std::io::ErrorKind::ConnectionReset => "连接被服务器重置".to_string(),
                    std::io::ErrorKind::ConnectionAborted => "连接被中止".to_string(),
                    std::io::ErrorKind::TimedOut => "连接超时".to_string(),
                    std::io::ErrorKind::WouldBlock => "操作会阻塞".to_string(),
                    std::io::ErrorKind::Interrupted => "操作被中断".to_string(),
                    _ => format!("读取错误: {}", e),
                };
                
                messages.lock().unwrap().push((get_timestamp(), error_msg));
                
                // 对于某些错误类型，我们可能想要尝试重新连接
                if matches!(e.kind(), 
                            std::io::ErrorKind::ConnectionReset | 
                            std::io::ErrorKind::ConnectionAborted | 
                            std::io::ErrorKind::BrokenPipe) {
                    messages.lock().unwrap().push((get_timestamp(), "连接中断".to_string()));
                }
                
                break;
            }
        }
    }
    
    messages.lock().unwrap().push((get_timestamp(), "数据接收通道已关闭".to_string()));
}