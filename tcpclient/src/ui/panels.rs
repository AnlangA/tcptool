use crate::app::{EncodingMode, TcpClientApp};
use crate::message::Message;
use crate::network::scanner::{is_valid_ip, is_valid_ip_range, is_valid_port, is_valid_port_range};
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
            ui.add(
                egui::TextEdit::singleline(&mut app.ip)
                    .desired_width(120.0)
                    .hint_text("输入服务器IP"),
            );
        });

        ui.add_space(5.0);

        ui.horizontal(|ui| {
            ui.strong("端口号:");
            ui.add(
                egui::TextEdit::singleline(&mut app.port)
                    .desired_width(120.0)
                    .hint_text("输入端口"),
            );
        });

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(5.0);

        // 添加数据编码模式选择
        ui.vertical(|ui| {
            ui.strong("数据编码模式:");
            ui.add_space(5.0);

            ui.horizontal(|ui| {
                // 当用户选择UTF-8模式
                if ui.radio_value(&mut app.encoding_mode, EncodingMode::Utf8, "UTF-8").clicked() {
                    // 同步到共享的编码模式
                    *app.shared_encoding_mode.lock().unwrap() = EncodingMode::Utf8;
                }

                // 当用户选择十六进制模式
                if ui.radio_value(&mut app.encoding_mode, EncodingMode::Hex, "十六进制(HEX)").clicked() {
                    // 同步到共享的编码模式
                    *app.shared_encoding_mode.lock().unwrap() = EncodingMode::Hex;
                }
            });
        });
    });

    ui.add_space(15.0);

    // 连接/断开按钮区域
    ui.vertical_centered(|ui| {
        if !app.is_connected {
            if ui
                .add(
                    egui::Button::new("连接")
                        .fill(egui::Color32::from_rgb(100, 150, 220))
                        .min_size(egui::vec2(100.0, 30.0)),
                )
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
            if ui
                .add(
                    egui::Button::new("断开")
                        .fill(egui::Color32::from_rgb(220, 100, 100))
                        .min_size(egui::vec2(100.0, 30.0)),
                )
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
            let status_text = if app.is_connected {
                "已连接"
            } else {
                "未连接"
            };
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
        if ui
            .button(if app.should_scroll_to_bottom {
                "📌 禁用自动滚动"
            } else {
                "📌 启用自动滚动"
            })
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

        scroll_area.show(ui, |ui| {
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
                    create_message_frame(item_bg).show(ui, |ui| {
                        ui.colored_label(color, text);
                    });
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
        // 根据编码模式显示不同的提示文本
        let hint_text = match app.encoding_mode {
            EncodingMode::Utf8 => "输入要发送的UTF-8消息...",
            EncodingMode::Hex => "输入要发送的十六进制数据(如: 48 65 6C 6C 6F)...",
        };

        let text_edit = egui::TextEdit::multiline(&mut app.send_text)
            .desired_width(f32::INFINITY)
            .desired_rows(3)
            .hint_text(hint_text);

        ui.add(text_edit);

        // 如果是十六进制模式，验证输入
        if app.encoding_mode == EncodingMode::Hex && !app.send_text.is_empty() {
            if !is_valid_hex_string(&app.send_text) {
                ui.add_space(5.0);
                ui.colored_label(
                    egui::Color32::from_rgb(220, 50, 50),
                    "无效的十六进制格式，请使用空格分隔的十六进制值(如: 48 65 6C 6C 6F)"
                );
            }
        }
    });
}

// 验证十六进制字符串是否有效
fn is_valid_hex_string(s: &str) -> bool {
    // 允许空格分隔的十六进制字符串
    let hex_str = s.replace(" ", "");

    // 如果去除空格后为空，则返回true
    if hex_str.is_empty() {
        return true;
    }

    // 检查长度是否为偶数
    if hex_str.len() % 2 != 0 {
        return false;
    }

    // 检查每个字符是否是有效的十六进制字符
    hex_str.chars().all(|c| c.is_digit(16))
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

            // 检查十六进制格式是否有效
            let hex_valid = if app.encoding_mode == EncodingMode::Hex && !app.send_text.is_empty() {
                is_valid_hex_string(&app.send_text)
            } else {
                true
            };

            // 发送按钮
            let send_enabled = !app.send_text.is_empty() && app.is_connected && hex_valid;
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
    if ui
        .add(
            egui::Button::new("清空")
                .fill(egui::Color32::from_rgb(150, 150, 150))
                .min_size(egui::vec2(80.0, 28.0)),
        )
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
    // 如果是十六进制模式，验证输入
    if app.encoding_mode == EncodingMode::Hex && !app.send_text.is_empty() {
        if !is_valid_hex_string(&app.send_text) {
            // 如果十六进制格式无效，不发送
            app.received_messages.lock().unwrap().push((
                get_timestamp(),
                "无法发送: 十六进制格式无效".to_string(),
            ));
            return;
        }
    }

    if let Some(tx) = &app.tx {
        let tx = tx.clone();
        let text = app.send_text.clone();
        let encoding_mode = app.encoding_mode;
        send_message(&tx, text, encoding_mode);
        app.send_text.clear();
    }
}

// IP扫描面板 - 全新设计的独立扫描界面
pub fn render_scan_panel(app: &mut TcpClientApp, ui: &mut egui::Ui) {
    // 渲染面板标题
    render_scan_panel_header(ui);

    // 渲染扫描结果
    render_scan_right_panel(app, ui);
}

// 渲染扫描面板标题
fn render_scan_panel_header(ui: &mut egui::Ui) {
    // 顶部标题和描述 - 使用更现代的设计
    let header_bg = egui::Color32::from_rgb(41, 128, 185); // 漆蓝色背景
    let header = egui::Frame::new()
        .fill(header_bg)
        .inner_margin(egui::vec2(20.0, 15.0))
        .outer_margin(egui::vec2(0.0, 0.0));

    header.show(ui, |ui| {
        ui.vertical_centered(|ui| {
            ui.horizontal(|ui| {
                ui.heading(
                    egui::RichText::new("IP扫描工具")
                        .color(egui::Color32::WHITE)
                        .size(24.0),
                );
            });
            ui.add_space(5.0);
            ui.label(
                egui::RichText::new("扫描网络中的开放端口，快速发现可用服务")
                    .color(egui::Color32::WHITE),
            );
        });
    });
    ui.add_space(15.0);
}

// 渲染扫描面板左侧内容
pub fn render_scan_left_panel(app: &mut TcpClientApp, ui: &mut egui::Ui) {
    ui.vertical(|ui| {

        // 扫描设置区域
        render_scan_settings(app, ui);

        // 添加使用说明
        render_scan_help_section(ui);
    });
}

// 渲染扫描设置区域
fn render_scan_settings(app: &mut TcpClientApp, ui: &mut egui::Ui) {
    let scan_frame = egui::Frame::new()
        .fill(egui::Color32::from_rgb(245, 245, 250))
        .inner_margin(egui::vec2(15.0, 15.0))
        .outer_margin(egui::vec2(0.0, 0.0))
        .corner_radius(8.0)
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(200)));

    scan_frame.show(ui, |ui| {
        // 设置区域标题
        ui.vertical_centered(|ui| {
            ui.add_space(5.0);
            ui.heading(
                egui::RichText::new("扫描设置")
                    .color(egui::Color32::from_rgb(41, 128, 185))
                    .size(18.0),
            );
        });
        ui.add_space(15.0);

        // IP和端口输入区域
        render_ip_port_inputs(app, ui);

        ui.add_space(15.0);

        // 扫描按钮
        render_scan_button(app, ui);

        // 扫描状态显示
        render_scan_status(app, ui);
    });
}

// 渲染IP和端口输入区域
fn render_ip_port_inputs(app: &mut TcpClientApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.add_space(5.0);
        ui.strong(egui::RichText::new("起始IP:").size(16.0));
        ui.add(
            egui::TextEdit::singleline(&mut app.start_ip)
                .desired_width(150.0)
                .hint_text("192.168.1.1")
                .margin(egui::vec2(8.0, 6.0))
                .text_color(egui::Color32::from_rgb(41, 128, 185)),
        );
    });

    ui.add_space(5.0);

    ui.horizontal(|ui| {
        ui.add_space(5.0);
        ui.strong(egui::RichText::new("结束IP:").size(16.0));
        ui.add(
            egui::TextEdit::singleline(&mut app.end_ip)
                .desired_width(150.0)
                .hint_text("192.168.1.255")
                .margin(egui::vec2(8.0, 6.0))
                .text_color(egui::Color32::from_rgb(41, 128, 185)),
        );
    });

    ui.add_space(5.0);

    ui.horizontal(|ui| {
        ui.add_space(5.0);
        ui.strong(egui::RichText::new("起始端口:").size(16.0));
        ui.add(
            egui::TextEdit::singleline(&mut app.start_port)
                .desired_width(150.0)
                .hint_text("8888")
                .margin(egui::vec2(8.0, 6.0))
                .text_color(egui::Color32::from_rgb(41, 128, 185)),
        );
    });

    ui.add_space(5.0);

    ui.horizontal(|ui| {
        ui.add_space(5.0);
        ui.strong(egui::RichText::new("结束端口:").size(16.0));
        ui.add(
            egui::TextEdit::singleline(&mut app.end_port)
                .desired_width(150.0)
                .hint_text("8889")
                .margin(egui::vec2(8.0, 6.0))
                .text_color(egui::Color32::from_rgb(41, 128, 185)),
        );
    });

    ui.add_space(5.0);

    ui.horizontal(|ui| {
        ui.add_space(5.0);
        ui.strong(egui::RichText::new("超时时间(ms):").size(16.0));
        ui.add(
            egui::TextEdit::singleline(&mut app.timeout_ms)
                .desired_width(150.0)
                .hint_text("500")
                .margin(egui::vec2(8.0, 6.0))
                .text_color(egui::Color32::from_rgb(41, 128, 185)),
        );
    });
}

// 渲染扫描按钮
fn render_scan_button(app: &mut TcpClientApp, ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        let button_text = if app.is_scanning {
            "停止扫描"
        } else {
            "开始扫描"
        };
        let button_color = if app.is_scanning {
            egui::Color32::from_rgb(220, 100, 100)
        } else {
            egui::Color32::from_rgb(100, 150, 220)
        };

        if ui
            .add(
                egui::Button::new(egui::RichText::new(button_text).size(16.0).strong())
                    .fill(button_color)
                    .min_size(egui::vec2(150.0, 40.0))
                    .corner_radius(6.0),
            )
            .clicked()
        {
            if !app.is_scanning {
                // 验证输入
                if is_valid_ip(&app.start_ip) && is_valid_ip(&app.end_ip) {
                    if is_valid_port(&app.start_port) && is_valid_port(&app.end_port) {
                        if is_valid_ip_range(&app.start_ip, &app.end_ip) {
                            if is_valid_port_range(&app.start_port, &app.end_port) {
                                if let (Ok(start_port), Ok(end_port)) = (app.start_port.parse::<u16>(), app.end_port.parse::<u16>()) {
                                    if let Some(tx) = &app.tx {
                                        let tx = tx.clone();
                                        let start_ip = app.start_ip.clone();
                                        let end_ip = app.end_ip.clone();

                                        // 验证超时时间
                                        if let Ok(timeout_ms) = app.timeout_ms.parse::<u64>() {
                                            // 发送扫描命令
                                            let scan_results = app.scan_results.clone();
                                            let scan_logs = app.scan_logs.clone();
                                            tokio::spawn(async move {
                                                let _ = tx
                                                    .send(Message::ScanIp(
                                                        start_ip,
                                                        end_ip,
                                                        start_port,
                                                        end_port,
                                                        timeout_ms,
                                                        scan_results,
                                                        scan_logs,
                                                    ))
                                                    .await;
                                            });

                                            app.is_scanning = true;
                                            app.scan_results.lock().unwrap().clear(); // 清空之前的结果
                                            app.scan_logs.lock().unwrap().clear(); // 清空之前的日志
                                        } else {
                                            // 超时时间格式错误
                                            let error_msg = "超时时间格式无效";
                                            let timestamp = get_timestamp();
                                            app.scan_logs
                                                .lock()
                                                .unwrap()
                                                .push((timestamp.clone(), error_msg.to_string()));
                                        }
                                    }
                                } else {
                                    // 端口格式错误
                                    let error_msg = "端口格式无效";
                                    let timestamp = get_timestamp();
                                    app.scan_logs
                                        .lock()
                                        .unwrap()
                                        .push((timestamp.clone(), error_msg.to_string()));
                                }
                            } else {
                                // 端口范围无效
                                let error_msg = "端口范围无效或超过最大扫描范围(1000个端口)";
                                let timestamp = get_timestamp();
                                app.scan_logs
                                    .lock()
                                    .unwrap()
                                    .push((timestamp.clone(), error_msg.to_string()));
                            }
                        } else {
                            // IP范围无效
                            let error_msg = "IP范围无效或超过最大扫描范围(1000个IP)";
                            let timestamp = get_timestamp();
                            app.scan_logs
                                .lock()
                                .unwrap()
                                .push((timestamp.clone(), error_msg.to_string()));
                        }
                    } else {
                        // 端口格式错误
                        let error_msg = "端口格式无效";
                        let timestamp = get_timestamp();
                        app.scan_logs
                            .lock()
                            .unwrap()
                            .push((timestamp.clone(), error_msg.to_string()));
                    }
                } else {
                    // IP格式错误
                    let error_msg = "IP地址格式无效";
                    let timestamp = get_timestamp();
                    app.scan_logs
                        .lock()
                        .unwrap()
                        .push((timestamp.clone(), error_msg.to_string()));
                }
            } else {
                // 停止扫描
                app.is_scanning = false;
                let cancel_msg = "用户取消扫描";
                let timestamp = get_timestamp();
                app.scan_logs
                    .lock()
                    .unwrap()
                    .push((timestamp.clone(), cancel_msg.to_string()));
            }
        }
    });
}

// 渲染扫描状态显示
fn render_scan_status(app: &mut TcpClientApp, ui: &mut egui::Ui) {
    ui.add_space(10.0);
    ui.separator();
    ui.add_space(5.0);

    ui.horizontal(|ui| {
        ui.strong("状态:");
        let status_text = if app.is_scanning {
            "正在扫描"
        } else {
            "就绪"
        };
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
}

// 渲染扫描帮助区域
fn render_scan_help_section(ui: &mut egui::Ui) {
    ui.add_space(15.0);
    let help_frame = egui::Frame::new()
        .fill(egui::Color32::from_rgb(253, 245, 230))
        .inner_margin(egui::vec2(15.0, 15.0))
        .outer_margin(egui::vec2(0.0, 0.0))
        .corner_radius(8.0)
        .stroke(egui::Stroke::new(
            1.0,
            egui::Color32::from_rgb(210, 180, 140),
        ));

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
            ui.label(egui::RichText::new("输入IP范围和端口范围后点击开始扫描。").color(tip_color));
        });
        ui.add_space(5.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("•").strong().color(tip_color));
            ui.label(egui::RichText::new("扫描结果将实时显示在右侧。").color(tip_color));
        });
        ui.add_space(5.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("•").strong().color(tip_color));
            ui.label(egui::RichText::new("最大扫描范围为1000个IP地址和1000个端口。").color(tip_color));
        });
        ui.add_space(5.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("•").strong().color(tip_color));
            ui.label(egui::RichText::new("多线程扫描可显著提高扫描速度。").color(tip_color));
        });
        ui.add_space(5.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("•").strong().color(tip_color));
            ui.label(egui::RichText::new("超时时间可调整扫描的等待时间，过短可能遗漏端口，过长会降低扫描速度。").color(tip_color));
        });
    });
}

// 渲染扫描面板右侧内容
fn render_scan_right_panel(app: &mut TcpClientApp, ui: &mut egui::Ui) {
    ui.vertical(|ui| {
        ui.set_width(ui.available_width());

        // 扫描结果区域
        render_scan_results(app, ui);
    });
}

// 渲染扫描结果区域
fn render_scan_results(app: &mut TcpClientApp, ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.heading(
            egui::RichText::new("扫描结果")
                .color(egui::Color32::from_rgb(39, 174, 96))
                .size(18.0),
        );
    });
    ui.add_space(5.0);

    let results_frame = egui::Frame::new()
        .fill(egui::Color32::from_rgb(250, 255, 250))
        .stroke(egui::Stroke::new(
            1.0,
            egui::Color32::from_rgb(200, 230, 200),
        ))
        .inner_margin(egui::vec2(15.0, 15.0))
        .outer_margin(egui::vec2(0.0, 5.0))
        .corner_radius(8.0);

    // 计算合适的区域大小
    let available_height = ui.available_height() * 0.7; // 结果区域占据60%的高度

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
                    create_message_frame(item_bg).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.add_space(5.0);
                            ui.label(
                                egui::RichText::new("✔")
                                    .size(16.0)
                                    .color(egui::Color32::from_rgb(0, 150, 0)),
                            );
                            ui.add_space(8.0);
                            ui.colored_label(egui::Color32::from_rgb(0, 100, 0), result);
                        });
                    });
                }
            }
        });
    });
}

// 渲染扫描日志区域
pub fn render_scan_logs(app: &mut TcpClientApp, ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.heading(
            egui::RichText::new("扫描日志")
                .color(egui::Color32::from_rgb(100, 120, 150))
                .size(18.0),
        );
    });
    ui.add_space(5.0);

    let logs_frame = egui::Frame::new()
        .fill(egui::Color32::from_rgb(245, 245, 250))
        .stroke(egui::Stroke::new(
            1.0,
            egui::Color32::from_rgb(200, 200, 230),
        ))
        .inner_margin(egui::vec2(15.0, 15.0))
        .outer_margin(egui::vec2(0.0, 5.0))
        .corner_radius(8.0);

    // 计算合适的区域大小
    let available_height = ui.available_height() - 20.0; // 减去一些边距

    logs_frame.show(ui, |ui| {
        // 使用滑动窗口
        let scroll_area = egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .stick_to_bottom(true)
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
                    create_message_frame(item_bg).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.add_space(5.0);
                            ui.label(
                                egui::RichText::new("•")
                                    .size(16.0)
                                    .color(egui::Color32::from_rgb(100, 100, 150)),
                            );
                            ui.add_space(8.0);
                            ui.label(
                                egui::RichText::new(format!("[{}]", timestamp))
                                    .size(14.0)
                                    .color(egui::Color32::from_rgb(100, 100, 150)),
                            );
                            ui.add_space(5.0);
                            ui.colored_label(egui::Color32::from_rgb(80, 80, 100), log);
                        });
                    });
                }
            }
        });
    });
}

// 获取时间戳函数
fn get_timestamp() -> String {
    use crate::utils::get_timestamp;
    get_timestamp()
}

// 发送消息的工具函数
pub fn send_message(tx: &mpsc::Sender<Message>, text: String, encoding_mode: EncodingMode) {
    let tx = tx.clone();
    tokio::spawn(async move {
        let _ = tx.send(Message::Send(text, encoding_mode)).await;
    });
}
