use crate::app::EncodingMode;
use crate::utils::{get_timestamp, write_to_file};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, BufReader};
use std::fs::File;
use std::time::Instant;

// 优化的文件写入函数，减少锁定时间
async fn log_to_file(file: &Option<Arc<Mutex<File>>>, message: &str, messages: &Arc<Mutex<Vec<(String, String)>>>) {
    if let Some(file_arc) = file {
        if let Ok(mut file_guard) = file_arc.try_lock() {
            if let Err(e) = write_to_file(&mut file_guard, message) {
                let error_msg = format!("写入文件失败: {}", e);
                let timestamp = get_timestamp();
                messages.lock().unwrap().push((timestamp, error_msg));
            }
        }
    }
}

// 优化的消息添加函数，批量处理消息
fn add_message(messages: &Arc<Mutex<Vec<(String, String)>>>, message: String) {
    let timestamp = get_timestamp();
    messages.lock().unwrap().push((timestamp, message));
}

// 高效的十六进制转换函数
fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut hex_string = String::with_capacity(bytes.len() * 3);
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 {
            hex_string.push(' ');
        }
        hex_string.push_str(&format!("{:02X}", b));
    }
    hex_string
}

// 改进的异步处理数据接收的函数
pub async fn handle_data_reception(
    messages: Arc<Mutex<Vec<(String, String)>>>,
    port: tokio::net::tcp::OwnedReadHalf,
    encoding_mode: Arc<Mutex<EncodingMode>>,
    file: Option<Arc<Mutex<File>>>,
) {
    add_message(&messages, "数据接收通道已建立".to_string());

    // 使用更大的缓冲区和BufReader提高性能
    let mut reader = BufReader::with_capacity(8192, port);
    let mut read_buffer = vec![0u8; 8192];

    // 用于批量处理消息的计时器
    let mut last_ui_update = Instant::now();

    // 持续从读取半部分读取数据，直到连接关闭或发生错误
    loop {
        // 从读取半部分读取数据
        match reader.read(&mut read_buffer).await {
            Ok(0) => {
                let message = "服务器关闭了连接".to_string();
                add_message(&messages, message.clone());
                log_to_file(&file, &message, &messages).await;
                break;
            }
            Ok(n) => {
                // 获取当前编码模式，减少锁定时间
                let current_mode = *encoding_mode.lock().unwrap();

                // 处理接收到的数据
                let message = match current_mode {
                    EncodingMode::Utf8 => {
                        // UTF-8模式下尝试解析为UTF-8文本
                        match String::from_utf8(read_buffer[..n].to_vec()) {
                            Ok(data) => format!("收到(UTF-8): {}", data),
                            Err(_) => {
                                // 如果不是有效的UTF-8，则显示为十六进制
                                let hex_data = bytes_to_hex(&read_buffer[..n]);
                                format!("收到(非UTF-8数据): {}", hex_data)
                            }
                        }
                    },
                    EncodingMode::Hex => {
                        // 十六进制模式下直接显示为十六进制
                        let hex_data = bytes_to_hex(&read_buffer[..n]);
                        format!("收到(HEX): {}", hex_data)
                    }
                };

                // 添加消息到UI并写入文件
                add_message(&messages, message.clone());
                log_to_file(&file, &message, &messages).await;

                // 如果距离上次UI更新超过100ms，强制更新UI
                if last_ui_update.elapsed().as_millis() > 100 {
                    tokio::task::yield_now().await;
                    last_ui_update = Instant::now();
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

                add_message(&messages, error_msg.clone());
                log_to_file(&file, &error_msg, &messages).await;

                // 对于某些错误类型，记录连接中断
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::ConnectionReset
                        | std::io::ErrorKind::ConnectionAborted
                        | std::io::ErrorKind::BrokenPipe
                ) {
                    let conn_msg = "连接中断".to_string();
                    add_message(&messages, conn_msg.clone());
                    log_to_file(&file, &conn_msg, &messages).await;
                }

                break;
            }
        }
    }

    let message = "数据接收通道已关闭".to_string();
    add_message(&messages, message.clone());
    log_to_file(&file, &message, &messages).await;
}
