mod app;
mod message;
mod network;
mod ui;
mod utils;

fn main() -> Result<(), eframe::Error> {
    // 设置tokio运行时
    let runtime = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    let _guard = runtime.enter();

    // 设置eframe选项
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 700.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("TCP 客户端"),
        ..Default::default()
    };

    // 运行应用
    eframe::run_native(
        "TCP 客户端",
        options,
        Box::new(|cc| Ok(Box::<app::TcpClientApp>::new(app::TcpClientApp::new(cc)))),
    )
}
