pub mod connection;
pub mod receiver;
pub mod scanner;

pub use connection::handle_network_communications;
pub use receiver::handle_data_reception;
// 导出扫描器模块的函数
