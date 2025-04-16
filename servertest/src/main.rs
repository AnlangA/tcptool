use std::error::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // 连接到服务器
    let server_addr = "127.0.0.1:8888";
    println!("尝试连接到服务器 {}", server_addr);

    match TcpStream::connect(server_addr).await {
        Ok(mut stream) => {
            println!("成功连接到服务器!");

            // 要发送的消息
            let message = "Hello, TCP Server!";
            println!("发送消息: {}", message);

            // 发送消息
            stream.write_all(message.as_bytes()).await?;

            // 接收响应
            let mut buffer = vec![0; 1024];
            let n = stream.read(&mut buffer).await?;

            if n > 0 {
                let response = String::from_utf8_lossy(&buffer[0..n]);
                println!("接收到响应: {}", response);
            } else {
                println!("服务器关闭了连接，没有接收到响应");
            }
        }
        Err(e) => {
            eprintln!("无法连接到服务器: {}", e);

            // 尝试备用地址
            let backup_addr = "0.0.0.0:8888";
            println!("尝试连接到备用地址 {}", backup_addr);

            match TcpStream::connect(backup_addr).await {
                Ok(mut stream) => {
                    println!("成功连接到备用服务器!");

                    // 要发送的消息
                    let message = "Hello, TCP Server!";
                    println!("发送消息: {}", message);

                    // 发送消息
                    stream.write_all(message.as_bytes()).await?;

                    // 接收响应
                    let mut buffer = vec![0; 1024];
                    let n = stream.read(&mut buffer).await?;

                    if n > 0 {
                        let response = String::from_utf8_lossy(&buffer[0..n]);
                        println!("接收到响应: {}", response);
                    } else {
                        println!("服务器关闭了连接，没有接收到响应");
                    }
                }
                Err(e) => {
                    eprintln!("无法连接到备用服务器: {}", e);
                    return Err(e.into());
                }
            }
        }
    }

    Ok(())
}
