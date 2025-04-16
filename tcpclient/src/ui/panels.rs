use crate::app::TcpClientApp;
use crate::message::Message;
use crate::network::scanner::{is_valid_ip, is_valid_port, is_valid_ip_range};
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
    // 渲染面板标题
    render_send_panel_header(ui);

    // 渲染消息输入区域
    render_message_input_area(app, ui);

    ui.add_space(10.0);

    // 渲染发送控制按钮
    render_send_controls(app, ui);
}

// 渲染发送面板标题
fn render_send_panel_header(ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.heading("发送消息");
    });
    ui.add_space(10.0);
}

// 渲染消息输入区域
fn render_message_input_area(app: &mut TcpClientApp, ui: &mut egui::Ui) {
    let input_frame = create_input_frame();

    input_frame.show(ui, |ui| {
        let text_edit = egui::TextEdit::multiline(&mut app.send_text)
            .desired_width(f32::INFINITY)
            .desired_rows(3)
            .hint_text("输入要发送的消息...");
        ui.add(text_edit);
    });
}

// 创建输入框架
fn create_input_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(egui::Color32::from_rgb(250, 250, 255))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(200)))
        .inner_margin(egui::vec2(10.0, 10.0))
}

// 渲染发送控制按钮
fn render_send_controls(app: &mut TcpClientApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // 清空按钮
            render_clear_button(app, ui);

            ui.add_space(10.0);

            // 发送按钮
            let send_enabled = !app.send_text.is_empty() && app.is_connected;
            let send_button = create_send_button();

            let send_response = if send_enabled {
                ui.add(send_button)
            } else {
                ui.add_enabled(false, send_button)
            };

            // 处理发送按钮点击
            if send_response.clicked() && send_enabled {
                handle_send_button_click(app);
            }
        });
    });
}

// 渲染清空按钮
fn render_clear_button(app: &mut TcpClientApp, ui: &mut egui::Ui) {
    if ui.add(egui::Button::new("清空")
        .fill(egui::Color32::from_rgb(150, 150, 150))
        .min_size(egui::vec2(80.0, 28.0)))
        .clicked()
    {
        app.send_text.clear();
    }
}

// 创建发送按钮
fn create_send_button() -> egui::Button<'static> {
    egui::Button::new("发送")
        .fill(egui::Color32::from_rgb(100, 150, 220))
        .min_size(egui::vec2(80.0, 28.0))
}

// 处理发送按钮点击
fn handle_send_button_click(app: &mut TcpClientApp) {
    if let Some(tx) = &app.tx {
        let tx = tx.clone();
        let text = app.send_text.clone();
        send_message(&tx, text);
        app.send_text.clear();
    }
}

// IP扫描面板 - 全新设计的独立扫描界面
pub fn render_scan_panel(app: &mut TcpClientApp, ui: &mut egui::Ui) {
    // 顶部标题和描述 - 使用更现代的设计
    let header_bg = egui::Color32::from_rgb(41, 128, 185); // 漆蓝色背景
    let header = egui::Frame::new()
        .fill(header_bg)
        .inner_margin(egui::vec2(20.0, 15.0))
        .outer_margin(egui::vec2(0.0, 0.0));

    header.show(ui, |ui| {
        ui.vertical_centered(|ui| {
            ui.horizontal(|ui| {
                ui.heading(egui::RichText::new("IP扫描工具").color(egui::Color32::WHITE).size(24.0));
            });
            ui.add_space(5.0);
            ui.label(egui::RichText::new("扫描网络中的开放端口，快速发现可用服务").color(egui::Color32::WHITE));
        });
    });
    ui.add_space(15.0);

    // 使用水平布局分为左右两个部分
    ui.horizontal(|ui| {
        // 左侧设置区域
        ui.vertical(|ui| {
            ui.set_width(ui.available_width() * 0.3);

            // 扫描设置区域 - 使用现代化设计
            let scan_frame = egui::Frame::new()
                .fill(egui::Color32::from_rgb(245, 245, 250))
                .inner_margin(egui::vec2(15.0, 15.0))
                .outer_margin(egui::vec2(0.0, 0.0))
                .corner_radius(8.0)
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(200)));

            scan_frame.show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.horizontal(|ui| {
                        ui.add_space(5.0);
                        ui.heading(egui::RichText::new("扫描设置").color(egui::Color32::from_rgb(41, 128, 185)).size(18.0));
                    });
                });
                ui.add_space(15.0);

                ui.horizontal(|ui| {
                    ui.add_space(5.0);
                    ui.strong(egui::RichText::new("起始IP:").size(16.0));
                    ui.add(egui::TextEdit::singleline(&mut app.start_ip)
                        .desired_width(150.0)
                        .hint_text("192.168.1.1")
                        .margin(egui::vec2(8.0, 6.0))
                        .text_color(egui::Color32::from_rgb(41, 128, 185)));
                });

                ui.add_space(5.0);

                ui.horizontal(|ui| {
                    ui.add_space(5.0);
                    ui.strong(egui::RichText::new("结束IP:").size(16.0));
                    ui.add(egui::TextEdit::singleline(&mut app.end_ip)
                        .desired_width(150.0)
                        .hint_text("192.168.1.255")
                        .margin(egui::vec2(8.0, 6.0))
                        .text_color(egui::Color32::from_rgb(41, 128, 185)));
                });

                ui.add_space(5.0);

                ui.horizontal(|ui| {
                    ui.add_space(5.0);
                    ui.strong(egui::RichText::new("扫描端口:").size(16.0));
                    ui.add(egui::TextEdit::singleline(&mut app.scan_port)
                        .desired_width(150.0)
                        .hint_text("8888")
                        .margin(egui::vec2(8.0, 6.0))
                        .text_color(egui::Color32::from_rgb(41, 128, 185)));
                });

                ui.add_space(15.0);

                // 扫描按钮
                ui.vertical_centered(|ui| {
                    let button_text = if app.is_scanning { "停止扫描" } else { "开始扫描" };
                    let button_color = if app.is_scanning {
                        egui::Color32::from_rgb(220, 100, 100)
                    } else {
                        egui::Color32::from_rgb(100, 150, 220)
                    };

                    if ui.add(egui::Button::new(egui::RichText::new(button_text).size(16.0).strong())
                        .fill(button_color)
                        .min_size(egui::vec2(150.0, 40.0))
                        .corner_radius(6.0))
                        .clicked()
                    {
                        if !app.is_scanning {
                            // 验证输入
                            if is_valid_ip(&app.start_ip) && is_valid_ip(&app.end_ip) && is_valid_port(&app.scan_port) {
                                if is_valid_ip_range(&app.start_ip, &app.end_ip) {
                                    if let Ok(port) = app.scan_port.parse::<u16>() {
                                        if let Some(tx) = &app.tx {
                                            let tx = tx.clone();
                                            let start_ip = app.start_ip.clone();
                                            let end_ip = app.end_ip.clone();

                                            // 发送扫描命令
                                            let scan_results = app.scan_results.clone();
                                            let scan_logs = app.scan_logs.clone();
                                            tokio::spawn(async move {
                                                let _ = tx.send(Message::ScanIp(start_ip, end_ip, port, scan_results, scan_logs)).await;
                                            });

                                            app.is_scanning = true;
                                            app.scan_results.lock().unwrap().clear(); // 清空之前的结果
                                            app.scan_logs.lock().unwrap().clear(); // 清空之前的日志
                                        }
                                    } else {
                                        // 端口格式错误
                                        let error_msg = "端口格式无效";
                                        let timestamp = get_timestamp();
                                        app.scan_logs.lock().unwrap().push((timestamp.clone(), error_msg.to_string()));
                                    }
                                } else {
                                    // IP范围无效
                                    let error_msg = "IP范围无效或超过最大扫描范围(1000个IP)";
                                    let timestamp = get_timestamp();
                                    app.scan_logs.lock().unwrap().push((timestamp.clone(), error_msg.to_string()));
                                }
                            } else {
                                // 输入格式错误
                                let error_msg = "IP地址或端口格式无效";
                                let timestamp = get_timestamp();
                                app.scan_logs.lock().unwrap().push((timestamp.clone(), error_msg.to_string()));
                            }
                        } else {
                            // 停止扫描
                            app.is_scanning = false;
                            let cancel_msg = "用户取消扫描";
                            let timestamp = get_timestamp();
                            app.scan_logs.lock().unwrap().push((timestamp.clone(), cancel_msg.to_string()));
                        }
                    }
                });

                // 扫描状态显示
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(5.0);

                ui.horizontal(|ui| {
                    ui.strong("状态:");
                    let status_text = if app.is_scanning { "正在扫描" } else { "就绪" };
                    let status_color = if app.is_scanning {
                        egui::Color32::from_rgb(40, 180, 40)
                    } else {
                        egui::Color32::from_rgb(100, 100, 100)
                    };
                    ui.colored_label(status_color, status_text);
                });

                // 扫描结果计数
                let result_count = app.scan_results.lock().unwrap().len();
                ui.horizontal(|ui| {
                    ui.strong("发现端口:");
                    ui.label(format!("{}", result_count));
                });
            });

            // 添加一些使用说明
            ui.add_space(15.0);
            let help_frame = egui::Frame::new()
                .fill(egui::Color32::from_rgb(253, 245, 230))
                .inner_margin(egui::vec2(15.0, 15.0))
                .outer_margin(egui::vec2(0.0, 0.0))
                .corner_radius(8.0)
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(210, 180, 140)));

            help_frame.show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.horizontal(|ui| {
                        ui.add_space(5.0);
                        let info_color = egui::Color32::from_rgb(210, 105, 30);
                        ui.label(egui::RichText::new("ℹ").size(20.0).color(info_color));
                        ui.add_space(8.0);
                        ui.heading(egui::RichText::new("使用说明").color(info_color).size(18.0));
                    });
                });
                ui.add_space(10.0);

                let tip_color = egui::Color32::from_rgb(160, 82, 45);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("•").strong().color(tip_color));
                    ui.label(egui::RichText::new("输入IP范围和端口后点击开始扫描。").color(tip_color));
                });
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("•").strong().color(tip_color));
                    ui.label(egui::RichText::new("扫描结果将实时显示在右侧。").color(tip_color));
                });
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("•").strong().color(tip_color));
                    ui.label(egui::RichText::new("最大扫描范围为1000个IP地址。").color(tip_color));
                });
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("•").strong().color(tip_color));
                    ui.label(egui::RichText::new("多线程扫描可显著提高扫描速度。").color(tip_color));
                });
            });
        });

        ui.separator();

        // 右侧结果区域
        ui.vertical(|ui| {
            ui.set_width(ui.available_width());

            // 扫描结果区域
            ui.vertical_centered(|ui| {
                ui.horizontal(|ui| {
                    ui.heading(egui::RichText::new("扫描结果").color(egui::Color32::from_rgb(39, 174, 96)).size(18.0));
                });
            });
            ui.add_space(5.0);

            let results_frame = egui::Frame::new()
                .fill(egui::Color32::from_rgb(250, 255, 250))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(200, 230, 200)))
                .inner_margin(egui::vec2(15.0, 15.0))
                .outer_margin(egui::vec2(0.0, 5.0))
                .corner_radius(8.0);

            // 计算合适的区域大小
            let available_height = ui.available_height() * 0.6; // 结果区域占据60%的高度

            results_frame.show(ui, |ui| {
                // 使用滑动窗口
                let scroll_area = egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .max_height(available_height)
                    .id_salt("scan_results_scroll_area");

                scroll_area.show(ui, |ui| {
                    let results = app.scan_results.lock().unwrap();
                    if results.is_empty() {
                        ui.vertical_centered(|ui| {
                            ui.add_space(10.0);
                            if app.is_scanning {
                                ui.weak("正在扫描中...");
                                // 添加加载动画
                                let time = ui.input(|i| i.time);
                                let n_dots = ((time * 2.0) as usize) % 4;
                                let dots = "..".chars().take(n_dots).collect::<String>();
                                ui.label(format!("IP扫描进行中{}", dots));
                            } else {
                                ui.weak("暂无扫描结果");
                                ui.label("开始扫描后将在此显示发现的开放端口");
                            }
                            ui.add_space(10.0);
                        });
                    } else {
                        // 设置列表最大高度
                        ui.set_min_height(available_height);

                        for result in results.iter() {
                            // 创建一个带背景色的结果行
                            let item_bg = egui::Color32::from_rgba_unmultiplied(230, 255, 230, 255);
                            create_message_frame(item_bg)
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.add_space(5.0);
                                        ui.label(egui::RichText::new("✔").size(16.0).color(egui::Color32::from_rgb(0, 150, 0)));
                                        ui.add_space(8.0);
                                        ui.colored_label(egui::Color32::from_rgb(0, 100, 0), result);
                                    });
                                });
                        }
                    }
                });
            });

            ui.add_space(10.0);

            // 扫描日志区域
            ui.vertical_centered(|ui| {
                ui.horizontal(|ui| {
                    ui.heading(egui::RichText::new("扫描日志").color(egui::Color32::from_rgb(100, 120, 150)).size(18.0));
                });
            });
            ui.add_space(5.0);

            let logs_frame = egui::Frame::new()
                .fill(egui::Color32::from_rgb(245, 245, 250))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(200, 200, 230)))
                .inner_margin(egui::vec2(15.0, 15.0))
                .outer_margin(egui::vec2(0.0, 5.0))
                .corner_radius(8.0);

            // 计算合适的区域大小
            let available_height = ui.available_height() - 20.0; // 减去一些边距

            logs_frame.show(ui, |ui| {
                // 使用滑动窗口
                let scroll_area = egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .max_height(available_height)
                    .id_salt("scan_logs_scroll_area");

                scroll_area.show(ui, |ui| {
                    let logs = app.scan_logs.lock().unwrap();
                    if logs.is_empty() {
                        ui.vertical_centered(|ui| {
                            ui.add_space(10.0);
                            ui.weak("暂无扫描日志");
                            ui.add_space(5.0);
                            ui.label("开始扫描后将在此显示详细日志");
                            ui.add_space(10.0);
                        });
                    } else {
                        // 设置列表最大高度
                        ui.set_min_height(available_height);

                        for (timestamp, log) in logs.iter() {
                            // 创建一个带背景色的日志行
                            let item_bg = egui::Color32::from_rgba_unmultiplied(245, 245, 250, 255);
                            create_message_frame(item_bg)
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.add_space(5.0);
                                        ui.label(egui::RichText::new("•").size(16.0).color(egui::Color32::from_rgb(100, 100, 150)));
                                        ui.add_space(8.0);
                                        ui.label(egui::RichText::new(format!("[{}]", timestamp)).size(14.0).color(egui::Color32::from_rgb(100, 100, 150)));
                                        ui.add_space(5.0);
                                        ui.colored_label(egui::Color32::from_rgb(80, 80, 100), log);
                                    });
                                });
                        }
                    }
                });
            });
        });
    });
}

// 获取时间戳函数
fn get_timestamp() -> String {
    use crate::utils::get_timestamp;
    get_timestamp()
}

// 发送消息的工具函数
pub fn send_message(tx: &mpsc::Sender<Message>, text: String) {
    let tx = tx.clone();
    tokio::spawn(async move {
        let _ = tx.send(Message::Send(text)).await;
    });
}