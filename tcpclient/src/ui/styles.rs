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
    if msg.starts_with("收到:") {
        egui::Color32::from_rgb(0, 100, 0)
    } else if msg.starts_with("已发送:") {
        egui::Color32::from_rgb(0, 0, 150)
    } else if msg.contains("失败") || msg.contains("错误") || msg.contains("中断") {
        egui::Color32::from_rgb(180, 0, 0)
    } else if msg.contains("连接到") {
        egui::Color32::from_rgb(0, 128, 128)
    } else {
        egui::Color32::GRAY
    }
}

// 获取消息背景颜色
pub fn get_message_background(msg: &str) -> egui::Color32 {
    if msg.starts_with("收到:") {
        egui::Color32::from_rgba_unmultiplied(230, 255, 230, 255)
    } else if msg.starts_with("已发送:") {
        egui::Color32::from_rgba_unmultiplied(230, 230, 255, 255)
    } else {
        egui::Color32::from_rgba_unmultiplied(245, 245, 250, 255)
    }
}
