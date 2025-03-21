pub mod connection;
pub mod receiver;

pub use connection::handle_network_communications;
pub use receiver::handle_data_reception;