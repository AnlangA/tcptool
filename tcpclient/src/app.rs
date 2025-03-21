use crate::message::Message;
use crate::network::handle_network_communications;
use crate::ui::panels::{render_messages_panel, render_send_panel, render_settings_panel};
use crate::ui::styles::setup_style;
use eframe::{egui, App, CreationContext, Frame};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

// 定义应用状态
pub struct TcpClientApp {
    pub ip: String,
    pub port: String,
    pub is_connected: bool,
    pub tx: Option<mpsc::Sender<Message>>,
    pub received_messages: Arc<Mutex<Vec<(String, String)>>>, // (时间戳, 消息)
    pub send_text: String,
    pub should_scroll_to_bottom: bool,
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
            should_scroll_to_bottom: true,
        }
    }
}

impl TcpClientApp {
    pub fn new(cc: &CreationContext<'_>) -> Self {
        // 设置UI样式
        setup_style(&cc.egui_ctx);

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
            should_scroll_to_bottom: true,
        }
    }
}

impl App for TcpClientApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        // 左侧面板 - 连接设置
        egui::SidePanel::left("settings_panel")
            .default_width(220.0)
            .resizable(true)
            .show(ctx, |ui| {
                render_settings_panel(self, ui);
            });
        
        // 中央面板 - 消息显示
        egui::CentralPanel::default().show(ctx, |ui| {
            render_messages_panel(self, ui);
        });
        
        // 底部面板 - 发送消息
        egui::TopBottomPanel::bottom("send_panel")
            .height_range(egui::Rangef::new(120.0, 180.0))
            .resizable(true)
            .show(ctx, |ui| {
                render_send_panel(self, ui);
            });
        
        // 强制每帧重绘，确保消息及时显示
        ctx.request_repaint();
    }
}