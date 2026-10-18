//! codex-wasm: the socket types wasm32-wasip1 lacks, so crates that name them still compile.
//! A browser tab has no sockets: every constructor fails with `Unsupported`, so no value of
//! these types ever exists and their methods are unreachable.

use crate::io::{ReadBuf, Ready};
use std::convert::Infallible;
use std::fmt;
use std::io;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::task::{Context, Poll};
use std::time::Duration;

fn unsupported() -> io::Error {
    io::Error::new(io::ErrorKind::Unsupported, "no sockets in a browser tab")
}

/// Stub of `tokio::net::TcpSocket`; see the module docs.
pub struct TcpSocket {
    never: Infallible,
}

impl fmt::Debug for TcpSocket {
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.never {}
    }
}

#[allow(missing_docs, unused_variables)]
impl TcpSocket {
    pub fn new_v4() -> io::Result<TcpSocket> { Err(unsupported()) }
    pub fn new_v6() -> io::Result<TcpSocket> { Err(unsupported()) }
    pub fn set_keepalive(&self, keepalive: bool) -> io::Result<()> { match self.never {} }
    pub fn keepalive(&self) -> io::Result<bool> { match self.never {} }
    pub fn set_reuseaddr(&self, reuseaddr: bool) -> io::Result<()> { match self.never {} }
    pub fn reuseaddr(&self) -> io::Result<bool> { match self.never {} }
    pub fn set_reuseport(&self, reuseport: bool) -> io::Result<()> { match self.never {} }
    pub fn set_send_buffer_size(&self, size: u32) -> io::Result<()> { match self.never {} }
    pub fn send_buffer_size(&self) -> io::Result<u32> { match self.never {} }
    pub fn set_recv_buffer_size(&self, size: u32) -> io::Result<()> { match self.never {} }
    pub fn recv_buffer_size(&self) -> io::Result<u32> { match self.never {} }
    pub fn set_linger(&self, dur: Option<Duration>) -> io::Result<()> { match self.never {} }
    pub fn set_nodelay(&self, nodelay: bool) -> io::Result<()> { match self.never {} }
    pub fn nodelay(&self) -> io::Result<bool> { match self.never {} }
    pub fn set_tos(&self, tos: u32) -> io::Result<()> { match self.never {} }
    pub fn bind_device(&self, interface: Option<&[u8]>) -> io::Result<()> { match self.never {} }
    pub fn local_addr(&self) -> io::Result<SocketAddr> { match self.never {} }
    pub fn take_error(&self) -> io::Result<Option<io::Error>> { match self.never {} }
    pub fn bind(&self, addr: SocketAddr) -> io::Result<()> { match self.never {} }
    pub async fn connect(self, addr: SocketAddr) -> io::Result<super::TcpStream> { match self.never {} }
    pub fn listen(self, backlog: u32) -> io::Result<super::TcpListener> { match self.never {} }
    pub fn from_std_stream(std_stream: std::net::TcpStream) -> TcpSocket {
        panic!("no sockets in a browser tab")
    }
}

/// Stub of `tokio::net::UdpSocket`; see the module docs.
pub struct UdpSocket {
    never: Infallible,
}

impl fmt::Debug for UdpSocket {
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.never {}
    }
}

#[allow(missing_docs, unused_variables)]
impl UdpSocket {
    pub async fn bind<A>(addr: A) -> io::Result<UdpSocket> { Err(unsupported()) }
    pub fn from_std(socket: std::net::UdpSocket) -> io::Result<UdpSocket> { Err(unsupported()) }
    pub fn into_std(self) -> io::Result<std::net::UdpSocket> { match self.never {} }
    pub fn local_addr(&self) -> io::Result<SocketAddr> { match self.never {} }
    pub fn peer_addr(&self) -> io::Result<SocketAddr> { match self.never {} }
    pub async fn connect<A>(&self, addr: A) -> io::Result<()> { match self.never {} }
    pub async fn ready(&self, interest: crate::io::Interest) -> io::Result<Ready> { match self.never {} }
    pub async fn writable(&self) -> io::Result<()> { match self.never {} }
    pub fn poll_send_ready(&self, cx: &mut Context<'_>) -> Poll<io::Result<()>> { match self.never {} }
    pub async fn send(&self, buf: &[u8]) -> io::Result<usize> { match self.never {} }
    pub fn poll_send(&self, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> { match self.never {} }
    pub fn try_send(&self, buf: &[u8]) -> io::Result<usize> { match self.never {} }
    pub async fn readable(&self) -> io::Result<()> { match self.never {} }
    pub fn poll_recv_ready(&self, cx: &mut Context<'_>) -> Poll<io::Result<()>> { match self.never {} }
    pub async fn recv(&self, buf: &mut [u8]) -> io::Result<usize> { match self.never {} }
    pub fn poll_recv(&self, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> { match self.never {} }
    pub fn try_recv(&self, buf: &mut [u8]) -> io::Result<usize> { match self.never {} }
    pub async fn send_to<A>(&self, buf: &[u8], target: A) -> io::Result<usize> { match self.never {} }
    pub fn poll_send_to(&self, cx: &mut Context<'_>, buf: &[u8], target: SocketAddr) -> Poll<io::Result<usize>> { match self.never {} }
    pub fn try_send_to(&self, buf: &[u8], target: SocketAddr) -> io::Result<usize> { match self.never {} }
    pub async fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> { match self.never {} }
    pub fn poll_recv_from(&self, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<SocketAddr>> { match self.never {} }
    pub fn try_recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> { match self.never {} }
    pub async fn peek_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> { match self.never {} }
    pub fn broadcast(&self) -> io::Result<bool> { match self.never {} }
    pub fn set_broadcast(&self, on: bool) -> io::Result<()> { match self.never {} }
    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> { match self.never {} }
    pub fn ttl(&self) -> io::Result<u32> { match self.never {} }
    pub fn set_multicast_loop_v4(&self, on: bool) -> io::Result<()> { match self.never {} }
    pub fn set_multicast_loop_v6(&self, on: bool) -> io::Result<()> { match self.never {} }
    pub fn set_multicast_ttl_v4(&self, ttl: u32) -> io::Result<()> { match self.never {} }
    pub fn join_multicast_v4(&self, multiaddr: Ipv4Addr, interface: Ipv4Addr) -> io::Result<()> { match self.never {} }
    pub fn join_multicast_v6(&self, multiaddr: &Ipv6Addr, interface: u32) -> io::Result<()> { match self.never {} }
    pub fn leave_multicast_v4(&self, multiaddr: Ipv4Addr, interface: Ipv4Addr) -> io::Result<()> { match self.never {} }
    pub fn leave_multicast_v6(&self, multiaddr: &Ipv6Addr, interface: u32) -> io::Result<()> { match self.never {} }
    pub fn take_error(&self) -> io::Result<Option<io::Error>> { match self.never {} }
}

/// Stub of `tokio::net::UnixStream`; see the module docs.
pub struct UnixStream {
    never: Infallible,
}

impl fmt::Debug for UnixStream {
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.never {}
    }
}

#[allow(missing_docs, unused_variables)]
impl UnixStream {
    pub async fn connect<P: AsRef<std::path::Path>>(path: P) -> io::Result<UnixStream> { Err(unsupported()) }
    pub fn pair() -> io::Result<(UnixStream, UnixStream)> { Err(unsupported()) }
    pub async fn readable(&self) -> io::Result<()> { match self.never {} }
    pub async fn writable(&self) -> io::Result<()> { match self.never {} }
    pub fn try_read(&self, buf: &mut [u8]) -> io::Result<usize> { match self.never {} }
    pub fn try_write(&self, buf: &[u8]) -> io::Result<usize> { match self.never {} }
}

impl crate::io::AsyncRead for UnixStream {
    fn poll_read(self: std::pin::Pin<&mut Self>, _: &mut Context<'_>, _: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        match self.never {}
    }
}

impl crate::io::AsyncWrite for UnixStream {
    fn poll_write(self: std::pin::Pin<&mut Self>, _: &mut Context<'_>, _: &[u8]) -> Poll<io::Result<usize>> {
        match self.never {}
    }
    fn poll_flush(self: std::pin::Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.never {}
    }
    fn poll_shutdown(self: std::pin::Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.never {}
    }
}

/// Stub of `tokio::net::UnixListener`; see the module docs.
pub struct UnixListener {
    never: Infallible,
}

impl fmt::Debug for UnixListener {
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.never {}
    }
}

#[allow(missing_docs, unused_variables)]
impl UnixListener {
    pub fn bind<P: AsRef<std::path::Path>>(path: P) -> io::Result<UnixListener> { Err(unsupported()) }
    pub async fn accept(&self) -> io::Result<(UnixStream, ())> { match self.never {} }
}
