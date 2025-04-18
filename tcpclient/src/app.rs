use crate::message::Message;
use crate::network::handle_network_communications;
use crate::ui::panels::{
    render_messages_panel, render_scan_left_panel, render_scan_logs, render_scan_panel,
    render_send_panel, render_settings_panel,
};
use crate::ui::styles::setup_style;
use eframe::{egui, App, CreationContext, Frame};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

// 定义应用状态
pub struct TcpClientApp {
    // 连接相关状态
    pub ip: String,
    pub port: String,
    pub is_connected: bool,
    pub tx: Option<mpsc::Sender<Message>>,
    pub received_messages: Arc<Mutex<Vec<(String, String)>>>, // (时间戳, 消息)
    pub send_text: String,
    pub should_scroll_to_bottom: bool,

    // IP扫描相关状态
    pub start_ip: String,
    pub end_ip: String,
    pub start_port: String,
    pub end_port: String,
    pub timeout_ms: String,
    pub is_scanning: bool,
    pub scan_results: Arc<Mutex<Vec<String>>>, // 扫描结果列表
    pub scan_logs: Arc<Mutex<Vec<(String, String)>>>, // 扫描日志列表 (时间戳, 日志内容)

    // 界面相关状态
    pub current_view: AppView, // 当前显示的界面
}

// 定义应用界面类型
#[derive(PartialEq, Clone, Copy)]
pub enum AppView {
    Connection, // 连接和数据界面
    Scan,       // 扫描界面
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

            // IP扫描相关状态初始化
            start_ip: "127.0.0.1".to_string(),
            end_ip: "127.0.0.10".to_string(),
            start_port: "8888".to_string(),
            end_port: "8889".to_string(),
            timeout_ms: "500".to_string(),
            is_scanning: false,
            scan_results: Arc::new(Mutex::new(Vec::new())),
            scan_logs: Arc::new(Mutex::new(Vec::new())),

            // 界面相关状态初始化
            current_view: AppView::Connection,
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
            is_connected: false,
            tx: Some(tx),
            received_messages,
            send_text: String::new(),
            should_scroll_to_bottom: true,

            // IP扫描相关状态初始化
            is_scanning: false,
            scan_results: Arc::new(Mutex::new(Vec::new())),
            scan_logs: Arc::new(Mutex::new(Vec::new())),

            // 界面相关状态初始化
            current_view: AppView::Connection,

            ..Default::default()
        }
    }

    /// 渲染连接界面
    fn render_connection_view(&mut self, ctx: &egui::Context) {
        // 左侧面板 - 连接设置
        egui::SidePanel::left("settings_panel")
            .default_width(220.0)
            .resizable(true)
            .show(ctx, |ui| {
                render_settings_panel(self, ui);
            });

        // 底部面板 - 发送消息
        egui::TopBottomPanel::bottom("send_panel")
            .height_range(egui::Rangef::new(120.0, 180.0))
            .resizable(true)
            .show(ctx, |ui| {
                render_send_panel(self, ui);
            });

        // 中央面板 - 消息显示
        egui::CentralPanel::default().show(ctx, |ui| {
            render_messages_panel(self, ui);
        });
    }

    /// 渲染IP扫描界面
    fn render_scan_view(&mut self, ctx: &egui::Context) {
        // 左侧面板 - 扫描设置
        egui::SidePanel::left("scan_settings_panel")
            .default_width(220.0)
            .resizable(true)
            .show(ctx, |ui| {
                render_scan_left_panel(self, ui);
            });

        //底部面板 - 扫描日志
        egui::TopBottomPanel::bottom("scan_logs_panel")
            .height_range(egui::Rangef::new(300.0, 400.0))
            .resizable(true)
            .show(ctx, |ui| {
                render_scan_logs(self, ui);
            });

        // 中央界面
        egui::CentralPanel::default().show(ctx, |ui| {
            render_scan_panel(self, ui);
        });
    }
}

impl App for TcpClientApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        // 顶部菜单栏 - 切换不同界面
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.current_view, AppView::Connection, "连接");
                ui.selectable_value(&mut self.current_view, AppView::Scan, "IP扫描");
            });
        });

        // 根据当前界面类型显示不同内容
        match self.current_view {
            AppView::Connection => self.render_connection_view(ctx),
            AppView::Scan => self.render_scan_view(ctx),
        }

        // 强制每帧重绘，确保消息及时显示
        ctx.request_repaint();
    }
}
