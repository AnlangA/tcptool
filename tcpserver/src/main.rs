use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // 尝试绑定到一个高端口（8888）来避免权限问题
    let addr = "127.0.0.1:8888";
    let listener = match TcpListener::bind(addr).await {
        Ok(listener) => {
            println!("Server running on {}", addr);
            listener
        },
        Err(e) => {
            eprintln!("无法绑定到 {}: {}", addr, e);
            eprintln!("尝试绑定到备用端口 0.0.0.0:8888（允许从任何网络接口访问）");
            
            // 尝试使用 0.0.0.0 而不是 127.0.0.1
            let backup_addr = "0.0.0.0:8888";
            match TcpListener::bind(backup_addr).await {
                Ok(listener) => {
                    println!("Server running on {}", backup_addr);
                    listener
                },
                Err(e) => {
                    eprintln!("无法绑定到 {}: {}", backup_addr, e);
                    return Err(e.into());
                }
            }
        }
    };

    // 循环接收新的连接
    loop {
        // 当有新连接时，获取stream和客户端地址
        let (socket, addr) = listener.accept().await?;
        println!("New client connected: {}", addr);

        // 为每个新连接创建一个新的任务
        tokio::spawn(async move {
            // 处理这个客户端连接
            if let Err(e) = process_socket(socket).await {
                eprintln!("Error processing connection from {}: {}", addr, e);
            }
        });
    }
}

// 处理单个客户端连接的函数
async fn process_socket(mut socket: TcpStream) -> Result<(), Box<dyn Error>> {
    let mut buffer = vec![0; 1024];

    // 循环读取客户端发送的数据
    loop {
        // 从socket中读取数据
        let n = socket.read(&mut buffer).await?;
        
        // 如果读取到0字节，表示客户端已关闭连接
        if n == 0 {
            println!("Client disconnected");
            return Ok(());
        }

        // 将收到的数据原样发送回客户端
        println!("Received {} bytes, echoing back: {}", n, String::from_utf8_lossy(&buffer[0..n]));
        socket.write_all(&buffer[0..n]).await?;
    }
}