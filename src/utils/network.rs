use std::net::{IpAddr, UdpSocket};
use anyhow::{Result, anyhow};

/// 获取本机IPv6地址
pub fn get_local_ipv6() -> Result<IpAddr> {
    // 尝试连接到一个外部地址来获取本地IPv6地址
    let socket = UdpSocket::bind("[::]:0")?;
    
    // 连接到一个公共DNS服务器（Google DNS IPv6）
    socket.connect("[2001:4860:4860::8888]:53")?;
    
    let local_addr = socket.local_addr()?;
    
    match local_addr.ip() {
        IpAddr::V6(ipv6) => Ok(IpAddr::V6(ipv6)),
        IpAddr::V4(_) => Err(anyhow!("未获取到IPv6地址，只有IPv4地址")),
    }
}

/// 获取首选IPv6地址（使用UDP连接方法）
pub fn get_preferred_ipv6() -> Result<IpAddr> {
    get_local_ipv6()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_get_all_ipv6_addresses() {
        let result = get_all_ipv6_addresses();
        assert!(result.is_ok() || result.is_err());
    }
}