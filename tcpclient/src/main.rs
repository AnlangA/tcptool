use eframe::{egui, App, Frame, CreationContext};
use chrono;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use egui::epaint::text::{FontInsert, InsertFontFamily};

// 定义消息类型
enum Message {
    Connect(String, u16),
    Disconnect,
    Send(String),
}

// 定义应用状态
struct TcpClientApp {
    ip: String,
    port: String,
    is_connected: bool,
    tx: Option<mpsc::Sender<Message>>,
    received_messages: Arc<Mutex<Vec<(String, String)>>>, // (时间戳, 消息)
    send_text: String,
}

impl Default for TcpClientApp {
    fn default() -> Self {
        Self {
            ip: "127.0.0.1".to_string(),
            port: "8888".to_string(),
            is_connected: false,
            tx: None,
            received_messages: Arc::new(Mutex::new(Vec::new())),
            send_text: String::new(),
        }
    }
}

impl TcpClientApp {
    fn new(cc: &CreationContext<'_>) -> Self {
        // 加载自定义宋体字体 - 直接从编译时嵌入字体
        cc.egui_ctx.add_font(FontInsert::new(
            "stsong",
            egui::FontData::from_static(include_bytes!("../font/STSong.ttf")),
            vec![
                InsertFontFamily {
                    family: egui::FontFamily::Proportional,
                    priority: egui::epaint::text::FontPriority::Highest,
                },
                InsertFontFamily {
                    family: egui::FontFamily::Monospace,
                    priority: egui::epaint::text::FontPriority::Highest,
                },
            ],
        ));
        
        // 设置应用样式
        let mut style = (*cc.egui_ctx.style()).clone();
        style.spacing.item_spacing = egui::vec2(10.0, 10.0);
        style.visuals = egui::Visuals::light(); // 使用浅色主题
        style.visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(240, 240, 245);
        style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(230, 230, 235);
        style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(210, 210, 220);
        style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(220, 220, 230);
        
        // 在eframe 0.31中，window_shadow的属性是不同类型的
        // 根据编译错误，需要使用正确的类型
        style.visuals.window_shadow.offset = [2, 2]; // 使用i8数组而不是vec2
        style.visuals.window_shadow.blur = 8; // 使用u8而不是f32
        style.visuals.window_shadow.spread = 1; // 使用u8而不是f32
        
        cc.egui_ctx.set_style(style);

        // 创建通信通道和共享状态
        let (tx, rx) = mpsc::channel::<Message>(100);
        let received_messages = Arc::new(Mutex::new(Vec::new()));
        
        // 启动异步任务处理网络通信
        let messages_clone = received_messages.clone();
        tokio::spawn(async move {
            handle_network_communications(rx, messages_clone).await;
        });

        Self {
            ip: "127.0.0.1".to_string(),
            port: "8888".to_string(),
            is_connected: false,
            tx: Some(tx),
            received_messages,
            send_text: String::new(),
        }
    }
}

// 获取当前时间字符串
fn get_timestamp() -> String {
    let now = std::time::SystemTime::now();
    let datetime = chrono::DateTime::<chrono::Local>::from(now);
    datetime.format("%H:%M:%S").to_string()
}

// 异步处理网络通信的函数
async fn handle_network_communications(
    mut rx: mpsc::Receiver<Message>,
    messages: Arc<Mutex<Vec<(String, String)>>>,
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
                        messages.lock().unwrap().push((get_timestamp(), format!("已连接到 {}", connect_addr)));
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
                        tokio::spawn(async move {
                            handle_data_reception(recv_messages, read_half).await;
                        });
                    }
                    Err(e) => {
                        messages.lock().unwrap().push((get_timestamp(), format!("连接失败: {}", e)));
                    }
                }
            }
            Message::Disconnect => {
                if has_connection {
                    // 清空通道
                    while conn_rx.try_recv().is_ok() {}
                    has_connection = false;
                    messages.lock().unwrap().push((get_timestamp(), "已断开连接".to_string()));
                }
            }
            Message::Send(data) => {
                if has_connection {
                    // 尝试从通道获取连接
                    match conn_rx.try_recv() {
                        Ok(mut stream) => {
                            let send_messages = messages.clone();
                            let send_data = data.clone();
                            let conn_tx_clone = conn_tx.clone();
                            
                            // 在单独的任务中发送数据
                            tokio::spawn(async move {
                                match stream.write_all(send_data.as_bytes()).await {
                                    Ok(_) => {
                                        send_messages.lock().unwrap().push((get_timestamp(), format!("已发送: {}", send_data)));
                                        // 将连接放回通道
                                        let _ = conn_tx_clone.send(stream).await;
                                    }
                                    Err(e) => {
                                        send_messages.lock().unwrap().push((get_timestamp(), format!("发送失败: {}", e)));
                                        // 发送失败，不放回通道
                                    }
                                }
                            });
                        }
                        Err(_) => {
                            // 通道中没有连接，可能正在被另一个任务使用
                            messages.lock().unwrap().push((get_timestamp(), "连接正忙，请稍后再试".to_string()));
                        }
                    }
                } else {
                    messages.lock().unwrap().push((get_timestamp(), "未连接，无法发送数据".to_string()));
                }
            }
        }
    }
}

// 改进的异步处理数据接收的函数
async fn handle_data_reception(
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

impl App for TcpClientApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        // 左侧面板 - 连接设置
        egui::SidePanel::left("settings_panel")
            .default_width(220.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("连接设置");
                });
                ui.add_space(15.0);
                
                // 使用eframe 0.31兼容的Frame创建方式
                // 根据编译错误，Frame::new()不接受任何参数
                let frame = egui::Frame::new()
                    .fill(egui::Color32::from_rgb(245, 245, 250))
                    .inner_margin(egui::vec2(10.0, 10.0));
                
                frame.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.strong("IP 地址:");
                        ui.add(egui::TextEdit::singleline(&mut self.ip)
                            .desired_width(120.0)
                            .hint_text("输入服务器IP"));
                    });
                    
                    ui.add_space(5.0);
                    
                    ui.horizontal(|ui| {
                        ui.strong("端口号:");
                        ui.add(egui::TextEdit::singleline(&mut self.port)
                            .desired_width(120.0)
                            .hint_text("输入端口"));
                    });
                });
                
                ui.add_space(15.0);
                
                // 连接/断开按钮区域
                ui.vertical_centered(|ui| {
                    if !self.is_connected {
                        if ui.add(egui::Button::new("连接")
                            .fill(egui::Color32::from_rgb(100, 150, 220))
                            .min_size(egui::vec2(100.0, 30.0)))
                            .clicked() 
                        {
                            if let Ok(port) = self.port.parse::<u16>() {
                                if let Some(tx) = &self.tx {
                                    let tx = tx.clone();
                                    let ip = self.ip.clone();
                                    tokio::spawn(async move {
                                        let _ = tx.send(Message::Connect(ip, port)).await;
                                    });
                                    self.is_connected = true;
                                }
                            }
                        }
                    } else {
                        if ui.add(egui::Button::new("断开")
                            .fill(egui::Color32::from_rgb(220, 100, 100))
                            .min_size(egui::vec2(100.0, 30.0)))
                            .clicked() 
                        {
                            if let Some(tx) = &self.tx {
                                let tx = tx.clone();
                                tokio::spawn(async move {
                                    let _ = tx.send(Message::Disconnect).await;
                                });
                                self.is_connected = false;
                            }
                        }
                    }
                });
                
                ui.add_space(20.0);
                ui.separator();
                
                // 连接状态显示
                let status_frame = egui::Frame::new()
                    .fill(egui::Color32::from_rgb(245, 245, 250))
                    .inner_margin(egui::vec2(10.0, 10.0));
                
                status_frame.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.strong("状态:");
                        let status_text = if self.is_connected { "已连接" } else { "未连接" };
                        let status_color = if self.is_connected { 
                            egui::Color32::from_rgb(40, 180, 40) 
                        } else { 
                            egui::Color32::from_rgb(180, 40, 40) 
                        };
                        ui.colored_label(status_color, status_text);
                    });
                    
                    ui.add_space(5.0);
                    
                    let msg_count = self.received_messages.lock().unwrap().len();
                    ui.horizontal(|ui| {
                        ui.strong("消息数量:");
                        ui.label(format!("{}", msg_count));
                    });
                });
            });
        
        // 中央面板 - 消息显示
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.heading("接收消息");
            });
            ui.add_space(10.0);
            
            // 创建带边框的滚动区域显示消息
            let messages_frame = egui::Frame::new()
                .fill(egui::Color32::from_rgb(250, 250, 255))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(200)))
                .inner_margin(egui::vec2(10.0, 10.0))
                .outer_margin(egui::vec2(0.0, 5.0));
                
            messages_frame.show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        let messages = self.received_messages.lock().unwrap();
                        if messages.is_empty() {
                            ui.weak("暂无消息...");
                        } else {
                            for (timestamp, msg) in messages.iter() {
                                // 根据消息类型添加不同的样式
                                let color = if msg.starts_with("收到:") {
                                    egui::Color32::from_rgb(0, 100, 0)
                                } else if msg.starts_with("已发送:") {
                                    egui::Color32::from_rgb(0, 0, 150)
                                } else if msg.contains("失败") || msg.contains("错误") || msg.contains("中断") {
                                    egui::Color32::from_rgb(180, 0, 0)
                                } else if msg.contains("连接到") {
                                    egui::Color32::from_rgb(0, 128, 128)
                                } else {
                                    egui::Color32::GRAY
                                };
                                
                                // 显示格式：[时间戳] 消息内容
                                let text = format!("[{}] {}", timestamp, msg);
                                
                                ui.colored_label(color, text);
                            }
                        }
                    });
            });
        });
        
        // 底部面板 - 发送消息
        egui::TopBottomPanel::bottom("send_panel")
            .height_range(egui::Rangef::new(120.0, 180.0))
            .resizable(true)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("发送消息");
                });
                ui.add_space(10.0);
                
                let input_frame = egui::Frame::new()
                    .fill(egui::Color32::from_rgb(250, 250, 255))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(200)))
                    .inner_margin(egui::vec2(10.0, 10.0));
                    
                input_frame.show(ui, |ui| {
                    let text_edit = egui::TextEdit::multiline(&mut self.send_text)
                        .desired_width(f32::INFINITY)
                        .desired_rows(3)
                        .hint_text("输入要发送的消息...");
                    ui.add(text_edit);
                });
                
                ui.add_space(10.0);
                
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::Button::new("清空")
                            .fill(egui::Color32::from_rgb(150, 150, 150))
                            .min_size(egui::vec2(80.0, 28.0)))
                            .clicked() 
                        {
                            self.send_text.clear();
                        }
                        
                        ui.add_space(10.0);
                        
                        let send_button = egui::Button::new("发送")
                            .fill(egui::Color32::from_rgb(100, 150, 220))
                            .min_size(egui::vec2(80.0, 28.0));
                        
                        let send_enabled = !self.send_text.is_empty() && self.is_connected;
                        let send_response = if send_enabled {
                            ui.add(send_button)
                        } else {
                            ui.add_enabled(false, send_button)
                        };
                        
                        if send_response.clicked() && send_enabled {
                            if let Some(tx) = &self.tx {
                                let tx = tx.clone();
                                let text = self.send_text.clone();
                                tokio::spawn(async move {
                                    let _ = tx.send(Message::Send(text)).await;
                                });
                                self.send_text.clear();
                            }
                        }
                    });
                });
            });
        
        // 强制每帧重绘，确保消息及时显示
        ctx.request_repaint();
    }
}

fn main() -> Result<(), eframe::Error> {
 
    // 设置tokio运行时
    let runtime = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    let _guard = runtime.enter();

    // 设置eframe选项
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 650.0])
            .with_min_inner_size([640.0, 480.0])
            .with_title("TCP 客户端"),
        ..Default::default()
    };
    
    // 运行应用
    eframe::run_native(
        "TCP 客户端",
        options,
        Box::new(|cc| Ok(Box::<TcpClientApp>::new(TcpClientApp::new(cc))))
    )
}