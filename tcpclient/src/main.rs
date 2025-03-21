use eframe::{egui, App, Frame, CreationContext};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};
use egui::epaint::text::{FontInsert, InsertFontFamily};
use std::time::{SystemTime, UNIX_EPOCH};

// 定义消息类型
enum Message {
    Connect {
        addr: String,
        port: u16,
        response: oneshot::Sender<bool>,
    },
    Disconnect,
    Send(String),
}

// 定义应用状态
struct TcpClientApp {
    ip: String,
    port: String,
    is_connected: bool,
    tx: mpsc::Sender<Message>,
    received_messages: Arc<Mutex<Vec<String>>>,
    send_text: String,
    connection_status: String,
    auto_scroll: bool,
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
            tx,
            received_messages,
            send_text: String::new(),
            connection_status: "未连接".to_string(),
            auto_scroll: true,
        }
    }

    // 添加消息到消息列表
    fn add_message(&self, message: String) {
        let mut messages = self.received_messages.lock().unwrap();
        let timestamp = format_timestamp();
        messages.push(format!("[{}] {}", timestamp, message));
        
        // 限制消息数量，防止内存无限增长
        if messages.len() > 1000 {
            messages.remove(0);
        }
    }
    
    // 清空消息历史
    fn clear_messages(&self) {
        let mut messages = self.received_messages.lock().unwrap();
        messages.clear();
    }
}

// 获取格式化的时间戳
fn format_timestamp() -> String {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = now.as_secs();
    let hours = (secs % 86400) / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

// 添加消息到消息队列的辅助函数
fn add_message(messages: &Arc<Mutex<Vec<String>>>, message: String) {
    let timestamp = format_timestamp();
    let mut messages = messages.lock().unwrap();
    messages.push(format!("[{}] {}", timestamp, message));
    
    // 限制消息数量，防止内存无限增长
    if messages.len() > 1000 {
        messages.remove(0);
    }
}

// 异步处理网络通信的函数
async fn handle_network_communications(
    mut rx: mpsc::Receiver<Message>,
    messages: Arc<Mutex<Vec<String>>>,
) {
    // 用于管理当前连接
    let mut write_conn: Option<TcpStream> = None;
    let mut read_conn_cancel: Option<oneshot::Sender<()>> = None;
    
    while let Some(msg) = rx.recv().await {
        match msg {
            Message::Connect { addr, port, response } => {
                // 如果已有连接，先断开
                if let Some(cancel) = read_conn_cancel.take() {
                    let _ = cancel.send(());
                }
                write_conn = None;
                
                let connect_addr = format!("{}:{}", addr, port);
                add_message(&messages, format!("正在连接到 {}...", connect_addr));
                
                // 尝试建立连接
                match TcpStream::connect(&connect_addr).await {
                    Ok(stream) => {
                        add_message(&messages, format!("已连接到 {}", connect_addr));
                        
                        // 设置连接属性
                        if let Err(e) = stream.set_nodelay(true) {
                            add_message(&messages, format!("设置TCP_NODELAY失败: {}", e));
                        }
                        
                        // 保存写入连接
                        write_conn = Some(stream);
                        
                        // 创建读取连接，将读写分开
                        match TcpStream::connect(&connect_addr).await {
                            Ok(read_stream) => {
                                // 创建取消通道
                                let (cancel_tx, mut cancel_rx) = oneshot::channel();
                                read_conn_cancel = Some(cancel_tx);
                                
                                // 启动读取任务
                                let read_messages = messages.clone();
                                tokio::spawn(async move {
                                    let mut read_stream = read_stream;
                                    let mut read_buffer = [0u8; 4096]; // 增大缓冲区
                                    
                                    loop {
                                        tokio::select! {
                                            // 检查是否收到取消信号
                                            _ = &mut cancel_rx => {
                                                add_message(&read_messages, "读取任务已取消".to_string());
                                                break;
                                            }
                                            // 读取数据
                                            read_result = read_stream.read(&mut read_buffer) => {
                                                match read_result {
                                                    Ok(0) => {
                                                        add_message(&read_messages, "服务器关闭了连接".to_string());
                                                        break;
                                                    }
                                                    Ok(n) => {
                                                        // 处理收到的数据
                                                        match String::from_utf8(read_buffer[..n].to_vec()) {
                                                            Ok(data) => {
                                                                add_message(&read_messages, format!("收到: {}", data));
                                                            }
                                                            Err(_) => {
                                                                // 处理非UTF-8数据
                                                                let hex_data: Vec<String> = read_buffer[..n]
                                                                    .iter()
                                                                    .map(|b| format!("{:02X}", b))
                                                                    .collect();
                                                                add_message(&read_messages, format!("收到二进制数据: {}", hex_data.join(" ")));
                                                            }
                                                        }
                                                    }
                                                    Err(e) => {
                                                        handle_connection_error(&read_messages, e);
                                                        break;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                });
                                
                                // 通知连接成功
                                let _ = response.send(true);
                            }
                            Err(e) => {
                                add_message(&messages, format!("创建读取连接失败: {}", e));
                                // 关闭写入连接
                                write_conn = None;
                                let _ = response.send(false);
                            }
                        }
                    }
                    Err(e) => {
                        add_message(&messages, format!("连接失败: {}", e));
                        let _ = response.send(false);
                    }
                }
            }
            Message::Disconnect => {
                // 断开连接
                if let Some(cancel) = read_conn_cancel.take() {
                    let _ = cancel.send(());
                }
                write_conn = None;
                add_message(&messages, "已断开连接".to_string());
            }
            Message::Send(data) => {
                // 发送数据
                if let Some(ref mut stream) = write_conn {
                    // 在当前任务中发送数据
                    match stream.write_all(data.as_bytes()).await {
                        Ok(_) => {
                            add_message(&messages, format!("已发送: {}", data));
                            
                            // 确保数据立即发送
                            if let Err(e) = stream.flush().await {
                                add_message(&messages, format!("刷新缓冲区失败: {}", e));
                            }
                        }
                        Err(e) => {
                            add_message(&messages, format!("发送失败: {}", e));
                            
                            // 如果发送失败，可能需要重新建立连接
                            if matches!(e.kind(), 
                                      std::io::ErrorKind::ConnectionReset | 
                                      std::io::ErrorKind::BrokenPipe | 
                                      std::io::ErrorKind::ConnectionAborted) {
                                // 清理连接
                                if let Some(cancel) = read_conn_cancel.take() {
                                    let _ = cancel.send(());
                                }
                                write_conn = None;
                                add_message(&messages, "连接已中断".to_string());
                            }
                        }
                    }
                } else {
                    add_message(&messages, "未连接，无法发送数据".to_string());
                }
            }
        }
    }
}

// 辅助函数: 处理连接错误
fn handle_connection_error(messages: &Arc<Mutex<Vec<String>>>, e: std::io::Error) {
    // 详细分类错误类型
    let error_msg = match e.kind() {
        std::io::ErrorKind::ConnectionReset => "连接被服务器重置".to_string(),
        std::io::ErrorKind::ConnectionAborted => "连接被中止".to_string(),
        std::io::ErrorKind::TimedOut => "连接超时".to_string(),
        std::io::ErrorKind::WouldBlock => "操作会阻塞".to_string(),
        std::io::ErrorKind::Interrupted => "操作被中断".to_string(),
        std::io::ErrorKind::BrokenPipe => "连接管道已断开".to_string(),
        _ => format!("读取错误: {}", e),
    };
    
    add_message(messages, error_msg);
}

impl App for TcpClientApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        // 左侧面板 - 连接设置
        egui::SidePanel::left("settings_panel")
            .resizable(true)
            .default_width(200.0)
            .show(ctx, |ui| {
                ui.heading("连接设置");
                ui.add_space(10.0);
                
                ui.horizontal(|ui| {
                    ui.label("IP 地址:");
                    ui.text_edit_singleline(&mut self.ip)
                        .on_hover_text("输入服务器IP地址");
                });
                
                ui.horizontal(|ui| {
                    ui.label("端口号:");
                    ui.text_edit_singleline(&mut self.port)
                        .on_hover_text("输入服务器端口号");
                });
                
                ui.add_space(10.0);
                
                ui.horizontal(|ui| {
                    if !self.is_connected {
                        if ui.button("连接").clicked() {
                            if let Ok(port) = self.port.parse::<u16>() {
                                // 创建oneshot通道以获取连接结果
                                let (resp_tx, resp_rx) = oneshot::channel();
                                
                                // 发送连接请求
                                let tx = self.tx.clone();
                                let ip = self.ip.clone();
                                tokio::spawn(async move {
                                    let _ = tx.send(Message::Connect {
                                        addr: ip,
                                        port,
                                        response: resp_tx,
                                    }).await;
                                });
                                
                                // 设置状态为连接中
                                self.connection_status = "连接中...".to_string();
                                
                                // 更新连接状态
                                tokio::spawn(async move {
                                    match resp_rx.await {
                                        Ok(true) => { /* 连接成功，状态会在消息处理中更新 */ }
                                        _ => { /* 连接失败，状态会在消息处理中更新 */ }
                                    }
                                });
                                
                                self.is_connected = true;
                            } else {
                                self.add_message("端口号格式不正确".to_string());
                            }
                        }
                    } else {
                        if ui.button("断开").clicked() {
                            let tx = self.tx.clone();
                            tokio::spawn(async move {
                                let _ = tx.send(Message::Disconnect).await;
                            });
                            self.is_connected = false;
                            self.connection_status = "未连接".to_string();
                        }
                    }
                    
                    if ui.button("清空消息").clicked() {
                        self.clear_messages();
                    }
                });
                
                ui.add_space(20.0);
                ui.separator();
                
                // 连接状态显示
                ui.horizontal(|ui| {
                    let status_text = format!("状态: {}", self.connection_status);
                    let status_color = if self.is_connected { 
                        egui::Color32::GREEN 
                    } else { 
                        egui::Color32::RED 
                    };
                    ui.colored_label(status_color, status_text);
                });
                
                let msg_count = self.received_messages.lock().unwrap().len();
                ui.label(format!("消息数量: {}", msg_count));
                
                ui.checkbox(&mut self.auto_scroll, "自动滚动到底部");
                
                ui.collapsing("帮助", |ui| {
                    ui.label("1. 输入服务器IP地址和端口号");
                    ui.label("2. 点击'连接'按钮建立TCP连接");
                    ui.label("3. 连接成功后可以在底部输入框发送消息");
                    ui.label("4. 收到的消息将显示在中央面板");
                    ui.label("5. 点击'断开'按钮关闭连接");
                });
            });
        
        // 中央面板 - 消息显示
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("接收消息");
            
            // 创建滚动区域显示消息
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .stick_to_bottom(self.auto_scroll)
                .show(ui, |ui| {
                    let messages = self.received_messages.lock().unwrap();
                    for msg in messages.iter() {
                        // 根据消息类型设置不同颜色
                        if msg.contains("已连接") || msg.contains("连接成功") {
                            ui.colored_label(egui::Color32::GREEN, msg);
                        } else if msg.contains("连接失败") || msg.contains("错误") || 
                                  msg.contains("中断") || msg.contains("断开") {
                            ui.colored_label(egui::Color32::RED, msg);
                        } else if msg.contains("收到:") {
                            ui.colored_label(egui::Color32::LIGHT_BLUE, msg);
                        } else if msg.contains("已发送:") {
                            ui.colored_label(egui::Color32::YELLOW, msg);
                        } else {
                            ui.label(msg);
                        }
                    }
                });
        });
        
        // 底部面板 - 发送消息
        egui::TopBottomPanel::bottom("send_panel")
            .resizable(true)
            .height_range(egui::Rangef::new(100.0, 200.0))
            .show(ctx, |ui| {
                ui.heading("发送消息");
                ui.add_space(5.0);
                
                let text_edit = egui::TextEdit::multiline(&mut self.send_text)
                    .desired_width(f32::INFINITY)
                    .desired_rows(3)
                    .hint_text("在此输入要发送的消息...");
                
                let response = ui.add(text_edit);
                
                // 支持Ctrl+Enter发送
                if (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.ctrl)) || 
                   (ui.button("发送").clicked() && !self.send_text.is_empty()) {
                    if self.is_connected {
                        let tx = self.tx.clone();
                        let text = self.send_text.clone();
                        tokio::spawn(async move {
                            let _ = tx.send(Message::Send(text)).await;
                        });
                        self.send_text.clear();
                    } else {
                        self.add_message("未连接，无法发送消息".to_string());
                    }
                }
                
                ui.horizontal(|ui| {
                    if ui.button("清空").clicked() {
                        self.send_text.clear();
                    }
                    
                    // 添加常用消息快捷发送按钮
                    ui.label("快捷消息:");
                    if ui.button("Ping").clicked() {
                        self.send_text = "PING".to_string();
                    }
                    if ui.button("Hello").clicked() {
                        self.send_text = "Hello, Server!".to_string();
                    }
                });
            });
        
        // 强制每帧重绘，确保消息及时显示，但限制帧率以减少CPU使用
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

fn main() -> Result<(), eframe::Error> {
    // 初始化日志（如果需要日志，请添加env_logger依赖）
    // env_logger::init();
    
    // 设置tokio运行时
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)  // 限制工作线程数量
        .enable_all()
        .build()
        .expect("Failed to create Tokio runtime");
    
    let _guard = runtime.enter();

    // 设置eframe选项
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_min_inner_size([400.0, 300.0])
            .with_title("TCP 客户端")
            .with_maximize_button(true)
            .with_decorations(true),
        vsync: true,  // 启用垂直同步以减少资源占用
        multisampling: 0,  // 禁用多重采样以提高性能
        ..Default::default()
    };
    
    // 运行应用
    eframe::run_native(
        "TCP 客户端",
        options,
        Box::new(|cc| Ok(Box::<TcpClientApp>::new(TcpClientApp::new(cc))))
    )
}