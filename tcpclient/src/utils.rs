use chrono;

// 获取当前时间字符串
pub fn get_timestamp() -> String {
    let now = std::time::SystemTime::now();
    let datetime = chrono::DateTime::<chrono::Local>::from(now);
    datetime.format("%H:%M:%S").to_string()
}