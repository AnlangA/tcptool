use crate::app::EncodingMode;
use crate::message::Message;
use crate::network::handle_data_reception;
use crate::network::scanner::scan_ip_range;
use crate::utils::get_timestamp;
use std::sync::{Arc, Mutex};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::mpsc;

// 异步处理网络通信的函数
pub async fn handle_network_communications(
    mut rx: mpsc::Receiver<Message>,
    messages: Arc<Mutex<Vec<(String, String)>>>,
    encoding_mode: Arc<Mutex<EncodingMode>>,
) {
    // 创建一个通道来管理TcpStream的所有权
    let (conn_tx, mut conn_rx) = mpsc::channel::<tokio::net::tcp::OwnedWriteHalf>(10);
    // 创建一个通道来管理TCP接收端口 - 修复未使用的变量警告
    let (_port_tx, _port_rx) = mpsc::channel::<tokio::net::tcp::OwnedReadHalf>(10);
    let mut has_connection = false;

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
                        messages
                            .lock()
                            .unwrap()
                            .push((get_timestamp(), format!("已连接到 {}", connect_addr)));
                        has_connection = true;

                        // 将stream分为发送和接收两个部分
                        let (read_half, write_half) = stream.into_split();

                        // 将新连接放入通道
                        let _ = conn_tx.send(write_half).await;

                        // 启动单独的异步任务处理数据接收
                        // 修复未使用的变量警告
                        let _recv_addr = connect_addr.clone();
                        let recv_messages = messages.clone();
                        let _conn_tx_clone = conn_tx.clone();
                        let recv_encoding_mode = encoding_mode.clone();
                        tokio::spawn(async move {
                            handle_data_reception(recv_messages, read_half, recv_encoding_mode).await;
                        });
                    }
                    Err(e) => {
                        messages
                            .lock()
                            .unwrap()
                            .push((get_timestamp(), format!("连接失败: {}", e)));
                    }
                }
            }
            Message::Disconnect => {
                if has_connection {
                    // 清空通道
                    while conn_rx.try_recv().is_ok() {}
                    has_connection = false;
                    messages
                        .lock()
                        .unwrap()
                        .push((get_timestamp(), "已断开连接".to_string()));
                }
            }
            Message::Send(data, encoding_mode) => {
                if has_connection {
                    // 尝试从通道获取连接
                    match conn_rx.try_recv() {
                        Ok(mut stream) => {
                            let send_messages = messages.clone();
                            let send_data = data.clone();
                            let conn_tx_clone = conn_tx.clone();

                            // 在单独的任务中发送数据
                            tokio::spawn(async move {
                                // 根据编码模式处理数据
                                let bytes_to_send = match encoding_mode {
                                    EncodingMode::Utf8 => send_data.as_bytes().to_vec(),
                                    EncodingMode::Hex => {
                                        // 将十六进制字符串转换为字节
                                        let mut bytes = Vec::new();
                                        let hex_str = send_data.replace(" ", ""); // 移除空格

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
                                };

                                match stream.write_all(&bytes_to_send).await {
                                    Ok(_) => {
                                        // 根据编码模式显示不同的消息
                                        let display_msg = match encoding_mode {
                                            EncodingMode::Utf8 => format!("已发送(UTF-8): {}", send_data),
                                            EncodingMode::Hex => format!("已发送(HEX): {}", send_data),
                                        };

                                        send_messages.lock().unwrap().push((
                                            get_timestamp(),
                                            display_msg,
                                        ));
                                        // 将连接放回通道
                                        let _ = conn_tx_clone.send(stream).await;
                                    }
                                    Err(e) => {
                                        send_messages
                                            .lock()
                                            .unwrap()
                                            .push((get_timestamp(), format!("发送失败: {}", e)));
                                        // 发送失败，不放回通道
                                    }
                                }
                            });
                        }
                        Err(_) => {
                            // 通道中没有连接，可能正在被另一个任务使用
                            messages
                                .lock()
                                .unwrap()
                                .push((get_timestamp(), "连接正忙，请稍后再试".to_string()));
                        }
                    }
                } else {
                    messages
                        .lock()
                        .unwrap()
                        .push((get_timestamp(), "未连接，无法发送数据".to_string()));
                }
            }
            Message::ScanIp(start_ip, end_ip, start_port, end_port, timeout_ms, scan_results, scan_logs) => {
                // 创建扫描任务
                let scan_messages = messages.clone();

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
                let timestamp = get_timestamp();

                scan_logs.lock().unwrap().push((timestamp, start_msg));

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
