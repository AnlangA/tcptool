use chrono;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

// 获取当前时间字符串 (用于UI显示)
pub fn get_timestamp() -> String {
    let now = std::time::SystemTime::now();
    let datetime = chrono::DateTime::<chrono::Local>::from(now);
    datetime.format("%H:%M:%S").to_string()
}

// 获取用于文件名的时间戳字符串
pub fn get_file_timestamp() -> String {
    let now = std::time::SystemTime::now();
    let datetime = chrono::DateTime::<chrono::Local>::from(now);
    datetime.format("%Y%m%d_%H%M%S").to_string()
}

// 创建并打开一个文件用于写入数据
pub fn create_data_file(ip: &str, port: u16) -> Result<(File, String), std::io::Error> {
    // 创建data目录（如果不存在）
    let data_dir = "data";
    if !Path::new(data_dir).exists() {
        fs::create_dir_all(data_dir)?;
    }

    // 生成文件名：ip_port_timestamp.txt
    let filename = format!("{}_{}_{}.txt", ip, port, get_file_timestamp());
    let filepath = format!("{}/{}", data_dir, filename);

    // 创建并打开文件
    let file = File::create(&filepath)?;

    Ok((file, filepath))
}

// 将数据写入文件
pub fn write_to_file(file: &mut File, data: &str) -> Result<(), std::io::Error> {
    writeln!(file, "[{}] {}", get_timestamp(), data)
}
