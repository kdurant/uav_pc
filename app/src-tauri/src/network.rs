use crate::protocol::Frame;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;

/// UDP 网络管理器
#[derive(Clone)]
pub struct UdpNetwork {
    socket: Arc<tokio::net::UdpSocket>,
    local_addr: SocketAddr,
}

impl UdpNetwork {
    /// 绑定本地 UDP 端口
    pub async fn bind(local_ip: &str, local_port: u16) -> Result<Self, String> {
        let addr = format!("{}:{}", local_ip, local_port);
        let socket = UdpSocket::bind(&addr)
            .await
            .map_err(|e| format!("Failed to bind UDP socket on {}: {}", addr, e))?;

        // 启用广播
        socket
            .set_broadcast(true)
            .map_err(|e| format!("Failed to set broadcast: {}", e))?;

        let local_addr = socket
            .local_addr()
            .map_err(|e| format!("Failed to get local addr: {}", e))?;

        log::info!("UDP socket bound to {}", local_addr);

        Ok(Self {
            socket: Arc::new(socket),
            local_addr,
        })
    }

    /// 获取本地地址
    #[allow(dead_code)]
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// 发送 UDP 广播到指定端口
    pub async fn send_broadcast(&self, frame: &Frame, port: u16) -> Result<(), String> {
        let broadcast_addr = format!("255.255.255.255:{}", port);
        let bytes = frame.to_bytes();
        self.socket
            .send_to(&bytes, &broadcast_addr)
            .await
            .map_err(|e| format!("Broadcast send failed: {}", e))?;
        Ok(())
    }

    /// 单播发送帧到指定地址
    pub async fn send_to(&self, target: &SocketAddr, frame: &Frame) -> Result<(), String> {
        let bytes = frame.to_bytes();
        self.socket
            .send_to(&bytes, target)
            .await
            .map_err(|e| format!("Unicast send to {} failed: {}", target, e))?;
        Ok(())
    }

    /// 异步接收帧
    pub async fn recv_from(&self) -> Result<(Frame, SocketAddr), String> {
        let mut buf = vec![0u8; 65536];
        let (len, addr) = self
            .socket
            .recv_from(&mut buf)
            .await
            .map_err(|e| format!("Receive failed: {}", e))?;

        let frame = Frame::from_bytes(&buf[..len])?;
        Ok((frame, addr))
    }
}
