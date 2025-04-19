use crate::utils::get_timestamp;
use futures::future::join_all;
use std::net::Ipv4Addr;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::net::TcpStream;
use tokio::task;
use tokio::time::{timeout, Duration};

// 将IP地址字符串转换为u32表示
fn ip_to_u32(ip: &str) -> Option<u32> {
    match Ipv4Addr::from_str(ip) {
        Ok(ipv4) => {
            let octets = ipv4.octets();
            Some(
                (octets[0] as u32) << 24
                    | (octets[1] as u32) << 16
                    | (octets[2] as u32) << 8
                    | (octets[3] as u32),
            )
        }
        Err(_) => None,
    }
}

// 将u32转换为IP地址字符串
fn u32_to_ip(ip: u32) -> String {
    let octet1 = (ip >> 24) & 0xFF;
    let octet2 = (ip >> 16) & 0xFF;
    let octet3 = (ip >> 8) & 0xFF;
    let octet4 = ip & 0xFF;
    format!("{}.{}.{}.{}", octet1, octet2, octet3, octet4)
}

// 检查IP地址是否有效
pub fn is_valid_ip(ip: &str) -> bool {
    Ipv4Addr::from_str(ip).is_ok()
}

// 检查端口是否有效
pub fn is_valid_port(port: &str) -> bool {
    match port.parse::<u16>() {
        Ok(_) => true,
        Err(_) => false,
    }
}

// 检查端口范围是否有效
pub fn is_valid_port_range(start_port: &str, end_port: &str) -> bool {
    if !is_valid_port(start_port) || !is_valid_port(end_port) {
        return false;
    }

    let start = start_port.parse::<u16>().unwrap();
    let end = end_port.parse::<u16>().unwrap();

    // 检查范围是否有效，并限制最大扫描范围为1000个端口
    start <= end && end - start <= 1000
}

// 检查IP范围是否有效
pub fn is_valid_ip_range(start_ip: &str, end_ip: &str) -> bool {
    if !is_valid_ip(start_ip) || !is_valid_ip(end_ip) {
        return false;
    }

    let start = ip_to_u32(start_ip);
    let end = ip_to_u32(end_ip);

    match (start, end) {
        (Some(s), Some(e)) => s <= e && e - s <= 1000, // 限制最大扫描范围为1000个IP
        _ => false,
    }
}

// 异步检查单个IP和端口是否开放
async fn check_port(ip: &str, port: u16, timeout_ms: u64) -> bool {
    let addr = format!("{}:{}", ip, port);
    match timeout(Duration::from_millis(timeout_ms), TcpStream::connect(&addr)).await {
        Ok(Ok(_)) => true,
        _ => false,
    }
}

// 并行扫描多个端口
async fn scan_ports(
    ip: &str,
    start_port: u16,
    end_port: u16,
    timeout_ms: u64,
    scan_results: &Arc<Mutex<Vec<String>>>,
    scan_logs: &Arc<Mutex<Vec<(String, String)>>>,
    open_ports: &Arc<AtomicUsize>,
    is_scanning: &Arc<Mutex<bool>>,
    is_cancelled: &Arc<AtomicBool>,
) -> usize {
    let mut found_count = 0;
    let mut port_tasks = Vec::new();
    let chunk_size = 50; // 每批并行扫描的端口数

    // 分批并行扫描端口
    for port_chunk_start in (start_port..=end_port).step_by(chunk_size) {
        let port_chunk_end = std::cmp::min(port_chunk_start + chunk_size as u16 - 1, end_port);

        for port in port_chunk_start..=port_chunk_end {
            // 检查是否取消扫描
            if !*is_scanning.lock().unwrap() || is_cancelled.load(Ordering::Relaxed) {
                is_cancelled.store(true, Ordering::Relaxed);
                return found_count;
            }

            let ip = ip.to_string();
            let scan_results = Arc::clone(scan_results);
            let scan_logs = Arc::clone(scan_logs);
            let open_ports = Arc::clone(open_ports);

            let task = tokio::spawn(async move {
                if check_port(&ip, port, timeout_ms).await {
                    open_ports.fetch_add(1, Ordering::Relaxed);
                    let result = format!("{} - 端口 {} 开放", ip, port);
                    scan_results.lock().unwrap().push(result.clone());

                    let found_msg = format!("发现开放端口: {}:{}", ip, port);
                    scan_logs.lock().unwrap().push((get_timestamp(), found_msg));
                    true
                } else {
                    false
                }
            });

            port_tasks.push(task);
        }

        // 等待当前批次完成
        for result in join_all(port_tasks).await {
            if let Ok(is_open) = result {
                if is_open {
                    found_count += 1;
                }
            }
        }

        // 重置任务列表为下一批次
        port_tasks = Vec::new();

        // 给系统一些时间处理其他任务
        tokio::task::yield_now().await;
    }

    found_count
}

// 执行IP扫描
pub async fn scan_ip_range(
    start_ip: &str,
    end_ip: &str,
    start_port: u16,
    end_port: u16,
    timeout_ms: u64,
    _messages: Arc<Mutex<Vec<(String, String)>>>,
    scan_results: Arc<Mutex<Vec<String>>>,
    scan_logs: Arc<Mutex<Vec<(String, String)>>>,
    is_scanning: Arc<Mutex<bool>>,
) {
    // 清空之前的扫描结果和日志
    scan_results.lock().unwrap().clear();
    scan_logs.lock().unwrap().clear();

    // 记录扫描开始
    let port_range_msg = if start_port == end_port {
        format!("端口: {}", start_port)
    } else {
        format!("端口范围: {} 到 {}", start_port, end_port)
    };

    let start_msg = format!("开始扫描IP范围: {} 到 {}, {}", start_ip, end_ip, port_range_msg);
    scan_logs.lock().unwrap().push((get_timestamp(), start_msg));

    // 转换IP地址为数字表示
    if let (Some(start), Some(end)) = (ip_to_u32(start_ip), ip_to_u32(end_ip)) {
        let total_ips = end - start + 1;
        let total_ports = (end_port - start_port + 1) as u32;
        let total_scans = total_ips * total_ports;
        let total_msg = format!("总共需要扫描 {} 个IP地址, {} 个端口, 共 {} 次扫描", total_ips, total_ports, total_scans);
        scan_logs.lock().unwrap().push((get_timestamp(), total_msg));

        // 使用原子计数器来跟踪进度和结果
        let scanned = Arc::new(AtomicUsize::new(0));
        let open_ports = Arc::new(AtomicUsize::new(0));
        let is_cancelled = Arc::new(AtomicBool::new(false));

        // 确定线程数量 - 根据IP数量和系统CPU核心数动态调整
        let cpu_cores = num_cpus::get();
        let total_ips_usize = total_ips as usize;
        let batch_size = std::cmp::max(1, total_ips_usize / cpu_cores);

        // 记录使用的线程数
        let thread_count = std::cmp::min(total_ips_usize, cpu_cores);
        let thread_msg = format!("使用 {} 个线程进行扫描", thread_count);
        scan_logs.lock().unwrap().push((get_timestamp(), thread_msg));

        // 创建任务集合
        let mut tasks = Vec::new();

        // 分批处理IP地址
        for batch_start in (start..=end).step_by(batch_size) {
            let batch_end = std::cmp::min(batch_start + batch_size as u32 - 1, end);

            // 克隆所有需要的引用
            let scan_results = Arc::clone(&scan_results);
            let scan_logs = Arc::clone(&scan_logs);
            let is_scanning = Arc::clone(&is_scanning);
            let scanned = Arc::clone(&scanned);
            let open_ports = Arc::clone(&open_ports);
            let is_cancelled = Arc::clone(&is_cancelled);
            let _batch_size = (batch_end - batch_start + 1) as usize;

            // 创建异步任务
            let task = task::spawn(async move {
                for ip_num in batch_start..=batch_end {
                    // 检查是否取消扫描
                    if !*is_scanning.lock().unwrap() || is_cancelled.load(Ordering::Relaxed) {
                        is_cancelled.store(true, Ordering::Relaxed);
                        break;
                    }

                    let ip_str = u32_to_ip(ip_num);
                    let current_scanned = scanned.fetch_add(1, Ordering::Relaxed) + 1;

                    // 更新进度 (每5个IP或批次结束时)
                    if current_scanned % 5 == 0 || current_scanned == total_ips_usize {
                        let progress_percent = (current_scanned * 100) / total_ips_usize;
                        let progress_msg = format!(
                            "扫描进度: {}/{} ({}%)",
                            current_scanned, total_ips_usize, progress_percent
                        );
                        scan_logs.lock().unwrap().push((get_timestamp(), progress_msg));
                    }

                    // 使用优化的端口扫描函数
                    scan_ports(
                        &ip_str,
                        start_port,
                        end_port,
                        timeout_ms,
                        &scan_results,
                        &scan_logs,
                        &open_ports,
                        &is_scanning,
                        &is_cancelled
                    ).await;
                }
            });

            tasks.push(task);
        }

        // 等待所有任务完成
        join_all(tasks).await;

        // 检查是否被取消
        if is_cancelled.load(Ordering::Relaxed) {
            let cancel_msg = "扫描已取消".to_string();
            scan_logs.lock().unwrap().push((get_timestamp(), cancel_msg));
        }

        // 获取最终计数
        let final_scanned = scanned.load(Ordering::Relaxed);
        let final_open_ports = open_ports.load(Ordering::Relaxed);

        // 记录扫描完成
        let complete_msg = format!(
            "扫描完成. 共扫描 {} 个IP, 发现 {} 个开放端口",
            final_scanned, final_open_ports
        );
        scan_logs.lock().unwrap().push((get_timestamp(), complete_msg));
    } else {
        let error_msg = "IP地址格式无效，无法开始扫描".to_string();
        scan_logs.lock().unwrap().push((get_timestamp(), error_msg));
    }

    // 标记扫描已完成
    *is_scanning.lock().unwrap() = false;
}

// 将扫描日志保存到文件 - 保留供将来使用
#[allow(dead_code)]
pub fn save_scan_logs_to_file(
    logs: &Vec<(String, String)>,
    file_path: &str,
) -> Result<(), std::io::Error> {
    use std::fs::File;
    use std::io::Write;

    let mut file = File::create(file_path)?;

    // 写入标题
    writeln!(file, "时间,日志内容")?;

    // 写入日志内容
    for (timestamp, message) in logs {
        writeln!(file, "{},{}", timestamp, message)?;
    }

    Ok(())
}
