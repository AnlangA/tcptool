use crate::app::TcpClientApp;
use crate::message::Message;
use crate::ui::styles::{create_message_frame, get_message_background, get_message_color};
use eframe::egui;
use tokio::sync::mpsc;

// 左侧设置面板
pub fn render_settings_panel(app: &mut TcpClientApp, ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.heading("连接设置");
    });
    ui.add_space(15.0);
    
    // 使用eframe 0.31兼容的Frame创建方式
    let frame = egui::Frame::new()
        .fill(egui::Color32::from_rgb(245, 245, 250))
        .inner_margin(egui::vec2(10.0, 10.0));
    
    frame.show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.strong("IP 地址:");
            ui.add(egui::TextEdit::singleline(&mut app.ip)
                .desired_width(120.0)
                .hint_text("输入服务器IP"));
        });
        
        ui.add_space(5.0);
        
        ui.horizontal(|ui| {
            ui.strong("端口号:");
            ui.add(egui::TextEdit::singleline(&mut app.port)
                .desired_width(120.0)
                .hint_text("输入端口"));
        });
    });
    
    ui.add_space(15.0);
    
    // 连接/断开按钮区域
    ui.vertical_centered(|ui| {
        if !app.is_connected {
            if ui.add(egui::Button::new("连接")
                .fill(egui::Color32::from_rgb(100, 150, 220))
                .min_size(egui::vec2(100.0, 30.0)))
                .clicked() 
            {
                if let Ok(port) = app.port.parse::<u16>() {
                    if let Some(tx) = &app.tx {
                        let tx = tx.clone();
                        let ip = app.ip.clone();
                        tokio::spawn(async move {
                            let _ = tx.send(Message::Connect(ip, port)).await;
                        });
                        app.is_connected = true;
                    }
                }
            }
        } else {
            if ui.add(egui::Button::new("断开")
                .fill(egui::Color32::from_rgb(220, 100, 100))
                .min_size(egui::vec2(100.0, 30.0)))
                .clicked() 
            {
                if let Some(tx) = &app.tx {
                    let tx = tx.clone();
                    tokio::spawn(async move {
                        let _ = tx.send(Message::Disconnect).await;
                    });
                    app.is_connected = false;
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
            let status_text = if app.is_connected { "已连接" } else { "未连接" };
            let status_color = if app.is_connected { 
                egui::Color32::from_rgb(40, 180, 40) 
            } else { 
                egui::Color32::from_rgb(180, 40, 40) 
            };
            ui.colored_label(status_color, status_text);
        });
        
        ui.add_space(5.0);
        
        let msg_count = app.received_messages.lock().unwrap().len();
        ui.horizontal(|ui| {
            ui.strong("消息数量:");
            ui.label(format!("{}", msg_count));
        });
    });
}

// 中央消息面板
pub fn render_messages_panel(app: &mut TcpClientApp, ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.heading("接收消息");
    });
    ui.add_space(10.0);
    
    // 添加一个自动滚动控制按钮
    ui.horizontal(|ui| {
        if ui.button(if app.should_scroll_to_bottom { "📌 禁用自动滚动" } else { "📌 启用自动滚动" })
            .clicked() 
        {
            app.should_scroll_to_bottom = !app.should_scroll_to_bottom;
        }
        
        if ui.button("🗑️ 清空消息").clicked() {
            app.received_messages.lock().unwrap().clear();
        }
    });
    
    // 创建带边框的滚动区域显示消息
    let messages_frame = egui::Frame::new()
        .fill(egui::Color32::from_rgb(250, 250, 255))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(200)))
        .inner_margin(egui::vec2(10.0, 10.0))
        .outer_margin(egui::vec2(0.0, 5.0));
        
    // 计算合适的区域大小
    let available_height = ui.available_height() - 20.0; // 减去一些边距
        
    messages_frame.show(ui, |ui| {
        // 使用滑动窗口，固定高度，自动滚动到底部
        let scroll_area = egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .stick_to_bottom(app.should_scroll_to_bottom)
            .max_height(available_height)
            .id_salt("messages_scroll_area");
            
        // 检查是否有新消息，如果有就设置自动滚动
        {
            let messages = app.received_messages.lock().unwrap();
            if !messages.is_empty() {
                if let Some(last_msg) = messages.last() {
                    // 如果最后一条消息的时间戳是在上一帧之后，激活自动滚动
                    let now = std::time::SystemTime::now();
                    let datetime = chrono::DateTime::<chrono::Local>::from(now);
                    let current_time = datetime.format("%H:%M:%S").to_string();
                    
                    // 简单比较时间戳字符串，如果最后一条消息是刚刚添加的，激活滚动
                    if last_msg.0 == current_time {
                        app.should_scroll_to_bottom = true;
                    }
                }
            }
        }
        
        scroll_area.show(ui, |ui| {
            // 当用户手动滚动时，禁用自动滚动
            if ui.input(|i| i.pointer.any_down() || i.pointer.any_pressed() || i.time_since_last_scroll() < 0.1) {
                app.should_scroll_to_bottom = false;
            }
                let messages = app.received_messages.lock().unwrap();
                if messages.is_empty() {
                    ui.weak("暂无消息...");
                } else {
                    // 设置列表最大高度
                    ui.set_min_height(available_height);
                    
                    for (timestamp, msg) in messages.iter() {
                        // 根据消息类型获取样式
                        let color = get_message_color(msg);
                        let item_bg = get_message_background(msg);
                        
                        // 显示格式：[时间戳] 消息内容
                        let text = format!("[{}] {}", timestamp, msg);
                        
                        // 创建一个带背景色的消息行
                        create_message_frame(item_bg)
                            .show(ui, |ui| {
                                ui.colored_label(color, text);
                            });
                        
                        // 如果启用了自动滚动，确保最后一条消息可见
                        if app.should_scroll_to_bottom && msg == &messages.last().unwrap().1 {
                            ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
                        }
                    }
                }
            });
    });
}

// 底部发送面板
pub fn render_send_panel(app: &mut TcpClientApp, ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.heading("发送消息");
    });
    ui.add_space(10.0);
    
    let input_frame = egui::Frame::new()
        .fill(egui::Color32::from_rgb(250, 250, 255))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(200)))
        .inner_margin(egui::vec2(10.0, 10.0));
        
    input_frame.show(ui, |ui| {
        let text_edit = egui::TextEdit::multiline(&mut app.send_text)
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
                app.send_text.clear();
            }
            
            ui.add_space(10.0);
            
            let send_button = egui::Button::new("发送")
                .fill(egui::Color32::from_rgb(100, 150, 220))
                .min_size(egui::vec2(80.0, 28.0));
            
            let send_enabled = !app.send_text.is_empty() && app.is_connected;
            let send_response = if send_enabled {
                ui.add(send_button)
            } else {
                ui.add_enabled(false, send_button)
            };
            
            if send_response.clicked() && send_enabled {
                if let Some(tx) = &app.tx {
                    let tx = tx.clone();
                    let text = app.send_text.clone();
                    send_message(&tx, text);
                    app.send_text.clear();
                }
            }
        });
    });
}

// 发送消息的工具函数
pub fn send_message(tx: &mpsc::Sender<Message>, text: String) {
    let tx = tx.clone();
    tokio::spawn(async move {
        let _ = tx.send(Message::Send(text)).await;
    });
}