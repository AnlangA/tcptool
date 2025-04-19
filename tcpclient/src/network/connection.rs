use crate::app::EncodingMode;
use crate::message::Message;
use crate::network::handle_data_reception;
use crate::network::scanner::scan_ip_range;
use crate::utils::{get_timestamp, create_data_file, write_to_file};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use std::time::Instant;

// 优化的消息添加函数，减少锁定时间
fn add_message(messages: &Arc<Mutex<Vec<(String, String)>>>, message: String) {
    let timestamp = get_timestamp();
    messages.lock().unwrap().push((timestamp, message));
}

// 优化的文件写入函数，减少锁定时间
async fn log_to_file(file: &Option<Arc<Mutex<std::fs::File>>>, message: &str, messages: &Arc<Mutex<Vec<(String, String)>>>) {
    if let Some(file_arc) = file {
        if let Ok(mut file_guard) = file_arc.try_lock() {
            if let Err(e) = write_to_file(&mut file_guard, message) {
                add_message(messages, format!("写入文件失败: {}", e));
            }
        }
    }
}

// 高效的十六进制转换函数
fn hex_to_bytes(hex_str: &str) -> Vec<u8> {
    let hex_str = hex_str.replace(" ", ""); // 移除空格
    let mut bytes = Vec::with_capacity(hex_str.len() / 2);

    // 每两个字符转换为一个字节
    for i in (0..hex_str.len()).step_by(2) {
        if i + 1 < hex_str.len() {
            if let Ok(byte) = u8::from_str_radix(&hex_str[i..i+2], 16) {
                bytes.push(byte);
            }
        }
    }
    bytes
}

// 异步处理网络通信的函数
pub async fn handle_network_communications(
    mut rx: mpsc::Receiver<Message>,
    messages: Arc<Mutex<Vec<(String, String)>>>,
    encoding_mode: Arc<Mutex<EncodingMode>>,
) {
    // 创建一个通道来管理TcpStream的所有权，增加缓冲区大小
    let (conn_tx, mut conn_rx) = mpsc::channel::<tokio::net::tcp::OwnedWriteHalf>(20);
    let mut has_connection = false;

    // 创建一个可选的文件句柄，用于在发送数据时使用
    let mut data_file: Option<Arc<Mutex<std::fs::File>>> = None;

    // 用于批量处理消息的计时器
    let mut last_ui_update = Instant::now();

    while let Some(msg) = rx.recv().await {
        match msg {
            Message::Connect(addr, port) => {
                // 如果已经连接，放弃现有连接
                has_connection = false;
                // 清空通道
                while conn_rx.try_recv().is_ok() {}

                let connect_addr = format!("{}:{}", addr, port);
                match TcpStream::connect(&connect_addr).await {
                    Ok(stream) => {
                        // 设置TCP选项以优化性能
                        if let Ok(socket) = stream.into_std() {
                            if let Err(e) = socket.set_nodelay(true) {
                                add_message(&messages, format!("设置TCP_NODELAY失败: {}", e));
                            }

                            // 转回TcpStream
                            let stream = TcpStream::from_std(socket).unwrap();
                            add_message(&messages, format!("已连接到 {}", connect_addr));
                            has_connection = true;

                            // 创建数据保存文件
                            let file_result = create_data_file(&addr, port);
                            match file_result {
                                Ok((file, filepath)) => {
                                    add_message(&messages, format!("创建数据文件: {}", filepath));

                                    // 将stream分为发送和接收两个部分
                                    let (read_half, write_half) = stream.into_split();

                                    // 将新连接放入通道
                                    let _ = conn_tx.send(write_half).await;

                                    // 创建文件句柄并保存到全局变量
                                    let file_arc = Arc::new(Mutex::new(file));
                                    data_file = Some(file_arc.clone());

                                    // 启动单独的异步任务处理数据接收
                                    let recv_messages = messages.clone();
                                    let recv_encoding_mode = encoding_mode.clone();
                                    tokio::spawn(async move {
                                        handle_data_reception(recv_messages, read_half, recv_encoding_mode, Some(file_arc)).await;
                                    });
                                },
                                Err(e) => {
                                    add_message(&messages, format!("创建数据文件失败: {}", e));

                                    // 将stream分为发送和接收两个部分
                                    let (read_half, write_half) = stream.into_split();

                                    // 将新连接放入通道
                                    let _ = conn_tx.send(write_half).await;

                                    // 启动单独的异步任务处理数据接收（不带文件）
                                    let recv_messages = messages.clone();
                                    let recv_encoding_mode = encoding_mode.clone();
                                    tokio::spawn(async move {
                                        handle_data_reception(recv_messages, read_half, recv_encoding_mode, None).await;
                                    });
                                }
                            }
                        } else {
                            add_message(&messages, "获取底层socket失败".to_string());
                        }
                    }
                    Err(e) => {
                        // 清除文件句柄
                        data_file = None;
                        add_message(&messages, format!("连接失败: {}", e));
                    }
                }
            }
            Message::Disconnect => {
                if has_connection {
                    // 清空通道
                    while conn_rx.try_recv().is_ok() {}
                    has_connection = false;

                    // 在文件中记录断开连接信息
                    let disconnect_msg = "已断开连接";
                    log_to_file(&data_file, disconnect_msg, &messages).await;
                    add_message(&messages, disconnect_msg.to_string());

                    // 清除文件句柄
                    data_file = None;
                }
            }
            Message::Send(data, encoding_mode) => {
                if has_connection {
                    // 尝试从通道获取连接
                    match conn_rx.try_recv() {
                        Ok(stream) => {
                            let send_messages = messages.clone();
                            let send_data = data.clone();
                            let conn_tx_clone = conn_tx.clone();
                            let file_clone = data_file.clone();

                            // 在单独的任务中发送数据
                            tokio::spawn(async move {
                                // 使用BufWriter提高写入性能
                                let mut writer = BufWriter::with_capacity(8192, stream);

                                // 根据编码模式处理数据
                                let bytes_to_send = match encoding_mode {
                                    EncodingMode::Utf8 => send_data.as_bytes().to_vec(),
                                    EncodingMode::Hex => hex_to_bytes(&send_data),
                                };

                                // 发送数据
                                let result = async {
                                    writer.write_all(&bytes_to_send).await?;
                                    writer.flush().await?;
                                    Ok::<_, std::io::Error>(writer.into_inner())
                                }.await;

                                match result {
                                    Ok(stream) => {
                                        // 根据编码模式显示不同的消息
                                        let display_msg = match encoding_mode {
                                            EncodingMode::Utf8 => format!("已发送(UTF-8): {}", send_data),
                                            EncodingMode::Hex => format!("已发送(HEX): {}", send_data),
                                        };

                                        // 将消息添加到UI显示
                                        add_message(&send_messages, display_msg.clone());

                                        // 如果有文件句柄，将发送的数据写入文件
                                        log_to_file(&file_clone, &display_msg, &send_messages).await;

                                        // 将连接放回通道
                                        let _ = conn_tx_clone.send(stream).await;
                                    }
                                    Err(e) => {
                                        add_message(&send_messages, format!("发送失败: {}", e));
                                        // 发送失败，不放回通道
                                    }
                                }
                            });
                        }
                        Err(_) => {
                            // 通道中没有连接，可能正在被另一个任务使用
                            add_message(&messages, "连接正忙，请稍后再试".to_string());
                        }
                    }
                } else {
                    add_message(&messages, "未连接，无法发送数据".to_string());
                }

                // 如果距离上次UI更新超过100ms，强制更新UI
                if last_ui_update.elapsed().as_millis() > 100 {
                    tokio::task::yield_now().await;
                    last_ui_update = Instant::now();
                }
            }
            Message::ScanIp(start_ip, end_ip, start_port, end_port, timeout_ms, scan_results, scan_logs) => {
                // 创建扫描状态标志
                let is_scanning = Arc::new(Mutex::new(true));

                // 记录扫描开始
                let port_range_msg = if start_port == end_port {
                    format!("端口: {}", start_port)
                } else {
                    format!("端口范围: {} 到 {}", start_port, end_port)
                };

                let start_msg = format!(
                    "IP扫描任务已启动: {} 到 {}, {}",
                    start_ip, end_ip, port_range_msg
                );

                scan_logs.lock().unwrap().push((get_timestamp(), start_msg));

                // 复制消息列表传递给扫描任务
                let scan_messages = messages.clone();

                // 启动扫描任务
                tokio::spawn(async move {
                    scan_ip_range(
                        &start_ip,
                        &end_ip,
                        start_port,
                        end_port,
                        timeout_ms,
                        scan_messages,
                        scan_results,
                        scan_logs,
                        is_scanning,
                    )
                    .await;
                });
            }
        }
    }
}
