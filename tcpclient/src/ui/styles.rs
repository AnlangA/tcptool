use eframe::egui;
use egui::epaint::text::{FontInsert, InsertFontFamily};

// 设置应用的UI样式
pub fn setup_style(ctx: &egui::Context) {
    // 加载自定义宋体字体 - 直接从编译时嵌入字体
    ctx.add_font(FontInsert::new(
        "stsong",
        egui::FontData::from_static(include_bytes!("../../font/STSong.ttf")),
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
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.visuals = egui::Visuals::light(); // 使用浅色主题
    style.visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(240, 240, 245);
    style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(230, 230, 235);
    style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(210, 210, 220);
    style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(220, 220, 230);

    // 在eframe 0.31中，window_shadow的属性是不同类型的
    style.visuals.window_shadow.offset = [2, 2]; // 使用i8数组而不是vec2
    style.visuals.window_shadow.blur = 8; // 使用u8而不是f32
    style.visuals.window_shadow.spread = 1; // 使用u8而不是f32

    ctx.set_style(style);
}

// 创建消息列表项框架
pub fn create_message_frame(item_bg: egui::Color32) -> egui::Frame {
    egui::Frame::new()
        .fill(item_bg)
        .inner_margin(egui::vec2(5.0, 3.0))
        .outer_margin(egui::vec2(0.0, 1.0))
}

// 获取消息颜色
pub fn get_message_color(msg: &str) -> egui::Color32 {
    if msg.starts_with("收到(UTF-8):") {
        egui::Color32::from_rgb(0, 120, 0) // 深绿色用于UTF-8接收消息
    } else if msg.starts_with("收到(HEX):") {
        egui::Color32::from_rgb(128, 0, 128) // 紫色用于十六进制接收消息
    } else if msg.starts_with("收到(非UTF-8数据):") {
        egui::Color32::from_rgb(160, 82, 45) // 棕色用于非UTF-8数据
    } else if msg.starts_with("收到:") {
        egui::Color32::from_rgb(0, 100, 0) // 原始的接收消息颜色
    } else if msg.starts_with("已发送(UTF-8):") {
        egui::Color32::from_rgb(0, 0, 180) // 蓝色用于UTF-8发送消息
    } else if msg.starts_with("已发送(HEX):") {
        egui::Color32::from_rgb(70, 30, 180) // 深蓝紫色用于十六进制发送消息
    } else if msg.starts_with("已发送:") {
        egui::Color32::from_rgb(0, 0, 150) // 原始的发送消息颜色
    } else if msg.contains("失败") || msg.contains("错误") || msg.contains("中断") {
        egui::Color32::from_rgb(180, 0, 0) // 红色用于错误消息
    } else if msg.contains("连接到") {
        egui::Color32::from_rgb(0, 128, 128) // 青色用于连接消息
    } else {
        egui::Color32::GRAY // 灰色用于其他消息
    }
}

// 获取消息背景颜色
pub fn get_message_background(msg: &str) -> egui::Color32 {
    if msg.starts_with("收到(UTF-8):") || msg.starts_with("收到:") {
        egui::Color32::from_rgba_unmultiplied(230, 255, 230, 255) // 浅绿色背景用于UTF-8接收消息
    } else if msg.starts_with("收到(HEX):") {
        egui::Color32::from_rgba_unmultiplied(245, 230, 255, 255) // 浅紫色背景用于十六进制接收消息
    } else if msg.starts_with("收到(非UTF-8数据):") {
        egui::Color32::from_rgba_unmultiplied(255, 240, 230, 255) // 浅棕色背景用于非UTF-8数据
    } else if msg.starts_with("已发送(UTF-8):") || msg.starts_with("已发送:") {
        egui::Color32::from_rgba_unmultiplied(230, 230, 255, 255) // 浅蓝色背景用于UTF-8发送消息
    } else if msg.starts_with("已发送(HEX):") {
        egui::Color32::from_rgba_unmultiplied(235, 230, 250, 255) // 浅蓝紫色背景用于十六进制发送消息
    } else if msg.contains("失败") || msg.contains("错误") || msg.contains("中断") {
        egui::Color32::from_rgba_unmultiplied(255, 230, 230, 255) // 浅红色背景用于错误消息
    } else {
        egui::Color32::from_rgba_unmultiplied(245, 245, 250, 255) // 浅灰色背景用于其他消息
    }
}
