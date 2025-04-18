use crate::app::EncodingMode;
use crate::utils::{get_timestamp, write_to_file};
use std::sync::{Arc, Mutex};
use tokio::io::AsyncReadExt;
use std::fs::File;

// 改进的异步处理数据接收的函数
pub async fn handle_data_reception(
    messages: Arc<Mutex<Vec<(String, String)>>>,
    mut port: tokio::net::tcp::OwnedReadHalf,
    encoding_mode: Arc<Mutex<EncodingMode>>,
    file: Option<Arc<Mutex<File>>>,
) {
    messages
        .lock()
        .unwrap()
        .push((get_timestamp(), "数据接收通道已建立".to_string()));

    let mut read_buffer = [0u8; 1024];

    // 持续从读取半部分读取数据，直到连接关闭或发生错误
    loop {
        // 从读取半部分读取数据
        match port.read(&mut read_buffer).await {
            Ok(0) => {
                let message = "服务器关闭了连接".to_string();
                messages
                    .lock()
                    .unwrap()
                    .push((get_timestamp(), message.clone()));

                // 如果有文件，将连接关闭信息写入文件
                if let Some(file_arc) = &file {
                    if let Ok(mut file_guard) = file_arc.lock() {
                        if let Err(e) = write_to_file(&mut file_guard, &message) {
                            messages
                                .lock()
                                .unwrap()
                                .push((get_timestamp(), format!("写入文件失败: {}", e)));
                        }
                    }
                }

                break;
            }
            Ok(n) => {
                // 根据当前编码模式处理收到的数据
                let current_mode = *encoding_mode.lock().unwrap();

                match current_mode {
                    EncodingMode::Utf8 => {
                        // UTF-8模式下尝试解析为UTF-8文本
                        match String::from_utf8(read_buffer[..n].to_vec()) {
                            Ok(data) => {
                                let message = format!("收到(UTF-8): {}", data);
                                messages
                                    .lock()
                                    .unwrap()
                                    .push((get_timestamp(), message.clone()));

                                // 如果有文件，将数据写入文件
                                if let Some(file_arc) = &file {
                                    if let Ok(mut file_guard) = file_arc.lock() {
                                        if let Err(e) = write_to_file(&mut file_guard, &message) {
                                            messages
                                                .lock()
                                                .unwrap()
                                                .push((get_timestamp(), format!("写入文件失败: {}", e)));
                                        }
                                    }
                                }
                            }
                            Err(_) => {
                                // 如果不是有效的UTF-8，则显示为十六进制
                                let hex_data: Vec<String> = read_buffer[..n]
                                    .iter()
                                    .map(|b| format!("{:02X}", b))
                                    .collect();
                                let message = format!("收到(非UTF-8数据): {}", hex_data.join(" "));
                                messages.lock().unwrap().push((
                                    get_timestamp(),
                                    message.clone(),
                                ));

                                // 如果有文件，将数据写入文件
                                if let Some(file_arc) = &file {
                                    if let Ok(mut file_guard) = file_arc.lock() {
                                        if let Err(e) = write_to_file(&mut file_guard, &message) {
                                            messages
                                                .lock()
                                                .unwrap()
                                                .push((get_timestamp(), format!("写入文件失败: {}", e)));
                                        }
                                    }
                                }
                            }
                        }
                    },
                    EncodingMode::Hex => {
                        // 十六进制模式下直接显示为十六进制，不尝试转换为文本
                        let hex_data: Vec<String> = read_buffer[..n]
                            .iter()
                            .map(|b| format!("{:02X}", b))
                            .collect();

                        let message = format!("收到(HEX): {}", hex_data.join(" "));
                        messages.lock().unwrap().push((
                            get_timestamp(),
                            message.clone(),
                        ));

                        // 如果有文件，将数据写入文件
                        if let Some(file_arc) = &file {
                            if let Ok(mut file_guard) = file_arc.lock() {
                                if let Err(e) = write_to_file(&mut file_guard, &message) {
                                    messages
                                        .lock()
                                        .unwrap()
                                        .push((get_timestamp(), format!("写入文件失败: {}", e)));
                                }
                            }
                        }
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

                messages.lock().unwrap().push((get_timestamp(), error_msg.clone()));

                // 如果有文件，将错误信息写入文件
                if let Some(file_arc) = &file {
                    if let Ok(mut file_guard) = file_arc.lock() {
                        if let Err(e) = write_to_file(&mut file_guard, &error_msg) {
                            messages
                                .lock()
                                .unwrap()
                                .push((get_timestamp(), format!("写入文件失败: {}", e)));
                        }
                    }
                }

                // 对于某些错误类型，我们可能想要尝试重新连接
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::ConnectionReset
                        | std::io::ErrorKind::ConnectionAborted
                        | std::io::ErrorKind::BrokenPipe
                ) {
                    let conn_msg = "连接中断".to_string();
                    messages
                        .lock()
                        .unwrap()
                        .push((get_timestamp(), conn_msg.clone()));

                    // 如果有文件，将连接中断信息写入文件
                    if let Some(file_arc) = &file {
                        if let Ok(mut file_guard) = file_arc.lock() {
                            if let Err(e) = write_to_file(&mut file_guard, &conn_msg) {
                                messages
                                    .lock()
                                    .unwrap()
                                    .push((get_timestamp(), format!("写入文件失败: {}", e)));
                            }
                        }
                    }
                }

                break;
            }
        }
    }

    let message = "数据接收通道已关闭".to_string();
    messages
        .lock()
        .unwrap()
        .push((get_timestamp(), message.clone()));

    // 如果有文件，将数据接收通道关闭信息写入文件
    if let Some(file_arc) = &file {
        if let Ok(mut file_guard) = file_arc.lock() {
            if let Err(e) = write_to_file(&mut file_guard, &message) {
                messages
                    .lock()
                    .unwrap()
                    .push((get_timestamp(), format!("写入文件失败: {}", e)));
            }
        }
    }
}
