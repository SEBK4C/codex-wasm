//! codex-wasm: socket2 0.6's API for wasm32-wasip1. A browser tab has no sockets, so every
//! constructor fails with `ErrorKind::Unsupported` and no `Socket` value ever exists. Crates
//! that name socket2 (hyper-util, rama) compile, and the code paths that would open a socket
//! never run in the tab: HTTP goes through the tab's `fetch` instead (see the reqwest fork).
#![allow(clippy::all, unused_variables, missing_docs, non_camel_case_types)]

use std::fmt;
use std::io;
use std::mem::MaybeUninit;
use std::net::{Ipv4Addr, Ipv6Addr, Shutdown, SocketAddr};
use std::time::Duration;

fn unsupported() -> io::Error {
    io::Error::new(io::ErrorKind::Unsupported, "no sockets in a browser tab")
}

pub type c_int = i32;
pub type sa_family_t = u16;
pub type socklen_t = u32;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct Domain(c_int);
impl Domain {
    pub const IPV4: Domain = Domain(2);
    pub const IPV6: Domain = Domain(10);
    pub const UNIX: Domain = Domain(1);
    pub const fn for_address(address: SocketAddr) -> Domain {
        match address {
            SocketAddr::V4(_) => Domain::IPV4,
            SocketAddr::V6(_) => Domain::IPV6,
        }
    }
}
impl From<c_int> for Domain { fn from(d: c_int) -> Domain { Domain(d) } }
impl From<Domain> for c_int { fn from(d: Domain) -> c_int { d.0 } }

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct Type(c_int);
impl Type {
    pub const STREAM: Type = Type(1);
    pub const DGRAM: Type = Type(2);
    pub const RAW: Type = Type(3);
    pub const SEQPACKET: Type = Type(5);
    pub const fn nonblocking(self) -> Type { self }
    pub const fn cloexec(self) -> Type { self }
}
impl From<c_int> for Type { fn from(t: c_int) -> Type { Type(t) } }
impl From<Type> for c_int { fn from(t: Type) -> c_int { t.0 } }

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct Protocol(c_int);
impl Protocol {
    pub const ICMPV4: Protocol = Protocol(1);
    pub const ICMPV6: Protocol = Protocol(58);
    pub const TCP: Protocol = Protocol(6);
    pub const UDP: Protocol = Protocol(17);
}
impl From<c_int> for Protocol { fn from(p: c_int) -> Protocol { Protocol(p) } }
impl From<Protocol> for c_int { fn from(p: Protocol) -> c_int { p.0 } }

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
pub struct RecvFlags(c_int);
impl RecvFlags {
    pub const fn is_truncated(self) -> bool { false }
}

#[derive(Clone, Debug, Default)]
pub struct TcpKeepalive {
    time: Option<Duration>,
    interval: Option<Duration>,
    retries: Option<u32>,
}
impl TcpKeepalive {
    pub const fn new() -> TcpKeepalive { TcpKeepalive { time: None, interval: None, retries: None } }
    pub const fn with_time(self, time: Duration) -> Self { TcpKeepalive { time: Some(time), ..self } }
    pub const fn with_interval(self, interval: Duration) -> Self { TcpKeepalive { interval: Some(interval), ..self } }
    pub const fn with_retries(self, retries: u32) -> Self { TcpKeepalive { retries: Some(retries), ..self } }
}

/// socket2's address type. Only conversions from and to `std::net::SocketAddr` work.
#[derive(Clone)]
pub struct SockAddr {
    addr: Option<SocketAddr>,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct SockAddrStorage { _bytes: [u8; 128] }
impl SockAddrStorage {
    pub fn zeroed() -> Self { SockAddrStorage { _bytes: [0; 128] } }
    pub fn size_of(&self) -> socklen_t { 128 }
}
impl fmt::Debug for SockAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "SockAddr({:?})", self.addr) }
}
impl SockAddr {
    pub fn unix<P: AsRef<std::path::Path>>(path: P) -> io::Result<SockAddr> { Err(unsupported()) }
    pub fn family(&self) -> sa_family_t {
        match self.addr { Some(SocketAddr::V4(_)) => 2, Some(SocketAddr::V6(_)) => 10, None => 0 }
    }
    pub fn domain(&self) -> Domain { Domain(self.family() as c_int) }
    pub fn len(&self) -> socklen_t { 0 }
    pub fn is_ipv4(&self) -> bool { matches!(self.addr, Some(SocketAddr::V4(_))) }
    pub fn is_ipv6(&self) -> bool { matches!(self.addr, Some(SocketAddr::V6(_))) }
    pub fn is_unix(&self) -> bool { false }
    pub fn as_socket(&self) -> Option<SocketAddr> { self.addr }
    pub fn as_socket_ipv4(&self) -> Option<std::net::SocketAddrV4> {
        match self.addr { Some(SocketAddr::V4(a)) => Some(a), _ => None }
    }
    pub fn as_socket_ipv6(&self) -> Option<std::net::SocketAddrV6> {
        match self.addr { Some(SocketAddr::V6(a)) => Some(a), _ => None }
    }
    pub fn as_pathname(&self) -> Option<&std::path::Path> { None }
    pub fn is_unnamed(&self) -> bool { false }
}
impl From<SocketAddr> for SockAddr { fn from(a: SocketAddr) -> SockAddr { SockAddr { addr: Some(a) } } }
impl From<std::net::SocketAddrV4> for SockAddr { fn from(a: std::net::SocketAddrV4) -> SockAddr { SockAddr { addr: Some(a.into()) } } }
impl From<std::net::SocketAddrV6> for SockAddr { fn from(a: std::net::SocketAddrV6) -> SockAddr { SockAddr { addr: Some(a.into()) } } }
impl PartialEq for SockAddr { fn eq(&self, o: &Self) -> bool { self.addr == o.addr } }
impl Eq for SockAddr {}
impl std::hash::Hash for SockAddr { fn hash<H: std::hash::Hasher>(&self, h: &mut H) { self.addr.hash(h) } }

/// A socket that can never be opened in a browser tab.
pub struct Socket {
    never: std::convert::Infallible,
}
impl fmt::Debug for Socket {
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result { match self.never {} }
}

macro_rules! never {
    ($($vis:vis fn $name:ident(&self $(, $arg:ident : $ty:ty)*) -> $ret:ty;)*) => {
        $( $vis fn $name(&self $(, $arg: $ty)*) -> $ret { match self.never {} } )*
    };
}

impl Socket {
    pub fn new(domain: Domain, ty: Type, protocol: Option<Protocol>) -> io::Result<Socket> { Err(unsupported()) }
    pub fn new_raw(domain: Domain, ty: Type, protocol: Option<Protocol>) -> io::Result<Socket> { Err(unsupported()) }
    pub fn pair(domain: Domain, ty: Type, protocol: Option<Protocol>) -> io::Result<(Socket, Socket)> { Err(unsupported()) }
    pub fn accept(&self) -> io::Result<(Socket, SockAddr)> { match self.never {} }
    pub fn try_clone(&self) -> io::Result<Socket> { match self.never {} }
    pub fn recv(&self, buf: &mut [MaybeUninit<u8>]) -> io::Result<usize> { match self.never {} }
    pub fn recv_from(&self, buf: &mut [MaybeUninit<u8>]) -> io::Result<(usize, SockAddr)> { match self.never {} }
    pub fn peek_from(&self, buf: &mut [MaybeUninit<u8>]) -> io::Result<(usize, SockAddr)> { match self.never {} }
    pub fn set_linger(&self, linger: Option<Duration>) -> io::Result<()> { match self.never {} }
    pub fn set_read_timeout(&self, d: Option<Duration>) -> io::Result<()> { match self.never {} }
    pub fn set_write_timeout(&self, d: Option<Duration>) -> io::Result<()> { match self.never {} }
    pub fn set_tcp_keepalive(&self, params: &TcpKeepalive) -> io::Result<()> { match self.never {} }
    pub fn bind_device(&self, interface: Option<&[u8]>) -> io::Result<()> { match self.never {} }
    pub fn device(&self) -> io::Result<Option<Vec<u8>>> { match self.never {} }
    pub fn linger(&self) -> io::Result<Option<Duration>> { match self.never {} }
    pub fn read_timeout(&self) -> io::Result<Option<Duration>> { match self.never {} }
    pub fn write_timeout(&self) -> io::Result<Option<Duration>> { match self.never {} }
    pub fn tcp_user_timeout(&self) -> io::Result<Option<Duration>> { match self.never {} }
    pub fn set_tcp_user_timeout(&self, timeout: Option<Duration>) -> io::Result<()> { match self.never {} }
    pub fn take_error(&self) -> io::Result<Option<io::Error>> { match self.never {} }
    pub fn join_multicast_v4(&self, multiaddr: &Ipv4Addr, interface: &Ipv4Addr) -> io::Result<()> { match self.never {} }
    pub fn join_multicast_v6(&self, multiaddr: &Ipv6Addr, interface: u32) -> io::Result<()> { match self.never {} }
    pub fn leave_multicast_v4(&self, multiaddr: &Ipv4Addr, interface: &Ipv4Addr) -> io::Result<()> { match self.never {} }
    pub fn leave_multicast_v6(&self, multiaddr: &Ipv6Addr, interface: u32) -> io::Result<()> { match self.never {} }
    pub fn multicast_if_v4(&self) -> io::Result<Ipv4Addr> { match self.never {} }
    pub fn set_multicast_if_v4(&self, interface: &Ipv4Addr) -> io::Result<()> { match self.never {} }
    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> { match self.never {} }
    never! {
        pub fn bind(&self, address: &SockAddr) -> io::Result<()>;
        pub fn connect(&self, address: &SockAddr) -> io::Result<()>;
        pub fn connect_timeout(&self, address: &SockAddr, timeout: Duration) -> io::Result<()>;
        pub fn listen(&self, backlog: c_int) -> io::Result<()>;
        pub fn local_addr(&self) -> io::Result<SockAddr>;
        pub fn peer_addr(&self) -> io::Result<SockAddr>;
        pub fn r#type(&self) -> io::Result<Type>;
        pub fn domain(&self) -> io::Result<Domain>;
        pub fn protocol(&self) -> io::Result<Option<Protocol>>;
        pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()>;
        pub fn send(&self, buf: &[u8]) -> io::Result<usize>;
        pub fn send_to(&self, buf: &[u8], addr: &SockAddr) -> io::Result<usize>;
        pub fn broadcast(&self) -> io::Result<bool>;
        pub fn set_broadcast(&self, broadcast: bool) -> io::Result<()>;
        pub fn keepalive(&self) -> io::Result<bool>;
        pub fn set_keepalive(&self, keepalive: bool) -> io::Result<()>;
        pub fn out_of_band_inline(&self) -> io::Result<bool>;
        pub fn set_out_of_band_inline(&self, oob_inline: bool) -> io::Result<()>;
        pub fn recv_buffer_size(&self) -> io::Result<usize>;
        pub fn set_recv_buffer_size(&self, size: usize) -> io::Result<()>;
        pub fn send_buffer_size(&self) -> io::Result<usize>;
        pub fn set_send_buffer_size(&self, size: usize) -> io::Result<()>;
        pub fn reuse_address(&self) -> io::Result<bool>;
        pub fn set_reuse_address(&self, reuse: bool) -> io::Result<()>;
        pub fn reuse_port(&self) -> io::Result<bool>;
        pub fn set_reuse_port(&self, reuse: bool) -> io::Result<()>;
        pub fn only_v6(&self) -> io::Result<bool>;
        pub fn set_only_v6(&self, only_v6: bool) -> io::Result<()>;
        pub fn ttl(&self) -> io::Result<u32>;
        pub fn set_ttl(&self, ttl: u32) -> io::Result<()>;
        pub fn ttl_v4(&self) -> io::Result<u32>;
        pub fn set_ttl_v4(&self, ttl: u32) -> io::Result<()>;
        pub fn tos(&self) -> io::Result<u32>;
        pub fn set_tos(&self, tos: u32) -> io::Result<()>;
        pub fn tos_v4(&self) -> io::Result<u32>;
        pub fn set_tos_v4(&self, tos: u32) -> io::Result<()>;
        pub fn tclass_v6(&self) -> io::Result<u32>;
        pub fn set_tclass_v6(&self, tclass: u32) -> io::Result<()>;
        pub fn unicast_hops_v6(&self) -> io::Result<u32>;
        pub fn set_unicast_hops_v6(&self, hops: u32) -> io::Result<()>;
        pub fn nodelay(&self) -> io::Result<bool>;
        pub fn set_nodelay(&self, nodelay: bool) -> io::Result<()>;
        pub fn tcp_nodelay(&self) -> io::Result<bool>;
        pub fn set_tcp_nodelay(&self, nodelay: bool) -> io::Result<()>;
        pub fn ip_transparent(&self) -> io::Result<bool>;
        pub fn set_ip_transparent(&self, transparent: bool) -> io::Result<()>;
        pub fn set_ip_transparent_v4(&self, transparent: bool) -> io::Result<()>;
        pub fn mark(&self) -> io::Result<u32>;
        pub fn set_mark(&self, mark: u32) -> io::Result<()>;
        pub fn multicast_loop_v4(&self) -> io::Result<bool>;
        pub fn set_multicast_loop_v4(&self, on: bool) -> io::Result<()>;
        pub fn multicast_loop_v6(&self) -> io::Result<bool>;
        pub fn set_multicast_loop_v6(&self, on: bool) -> io::Result<()>;
        pub fn multicast_ttl_v4(&self) -> io::Result<u32>;
        pub fn set_multicast_ttl_v4(&self, ttl: u32) -> io::Result<()>;
        pub fn multicast_hops_v6(&self) -> io::Result<u32>;
        pub fn set_multicast_hops_v6(&self, hops: u32) -> io::Result<()>;
        pub fn multicast_if_v6(&self) -> io::Result<u32>;
        pub fn set_multicast_if_v6(&self, interface: u32) -> io::Result<()>;
        pub fn header_included_v4(&self) -> io::Result<bool>;
        pub fn header_included_v6(&self) -> io::Result<bool>;
        pub fn set_header_included_v6(&self, included: bool) -> io::Result<()>;
        pub fn recv_hoplimit_v6(&self) -> io::Result<bool>;
        pub fn set_recv_hoplimit_v6(&self, recv: bool) -> io::Result<()>;
        pub fn recv_tclass_v6(&self) -> io::Result<bool>;
        pub fn set_recv_tclass_v6(&self, recv: bool) -> io::Result<()>;
        pub fn set_header_included_v4(&self, included: bool) -> io::Result<()>;
        pub fn cork(&self) -> io::Result<bool>;
        pub fn set_cork(&self, cork: bool) -> io::Result<()>;
        pub fn quickack(&self) -> io::Result<bool>;
        pub fn set_quickack(&self, quickack: bool) -> io::Result<()>;
        pub fn freebind(&self) -> io::Result<bool>;
        pub fn set_freebind(&self, freebind: bool) -> io::Result<()>;
        pub fn set_freebind_v4(&self, freebind: bool) -> io::Result<()>;
        pub fn set_freebind_v6(&self, freebind: bool) -> io::Result<()>;
        pub fn tcp_mss(&self) -> io::Result<u32>;
        pub fn set_tcp_mss(&self, mss: u32) -> io::Result<()>;
        pub fn recv_tos_v4(&self) -> io::Result<bool>;
        pub fn set_recv_tos_v4(&self, recv_tos: bool) -> io::Result<()>;
        pub fn tcp_congestion(&self) -> io::Result<Vec<u8>>;
        pub fn set_tcp_congestion(&self, tcp_ca_name: &[u8]) -> io::Result<()>;
    }
}

impl io::Read for Socket { fn read(&mut self, _: &mut [u8]) -> io::Result<usize> { match self.never {} } }
impl io::Write for Socket {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> { match self.never {} }
    fn flush(&mut self) -> io::Result<()> { match self.never {} }
}
impl<'a> io::Read for &'a Socket { fn read(&mut self, _: &mut [u8]) -> io::Result<usize> { match self.never {} } }
impl<'a> io::Write for &'a Socket {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> { match self.never {} }
    fn flush(&mut self) -> io::Result<()> { match self.never {} }
}

macro_rules! convert {
    ($($std:ty),*) => {$(
        impl From<$std> for Socket {
            fn from(_: $std) -> Socket { panic!("no sockets in a browser tab") }
        }
        impl From<Socket> for $std {
            fn from(s: Socket) -> $std { match s.never {} }
        }
    )*};
}
convert!(std::net::TcpStream, std::net::TcpListener, std::net::UdpSocket);

impl std::os::fd::AsFd for Socket {
    fn as_fd(&self) -> std::os::fd::BorrowedFd<'_> { match self.never {} }
}
impl std::os::fd::AsRawFd for Socket {
    fn as_raw_fd(&self) -> std::os::fd::RawFd { match self.never {} }
}
impl std::os::fd::IntoRawFd for Socket {
    fn into_raw_fd(self) -> std::os::fd::RawFd { match self.never {} }
}
impl std::os::fd::FromRawFd for Socket {
    unsafe fn from_raw_fd(_: std::os::fd::RawFd) -> Socket { panic!("no sockets in a browser tab") }
}
impl From<std::os::fd::OwnedFd> for Socket {
    fn from(_: std::os::fd::OwnedFd) -> Socket { panic!("no sockets in a browser tab") }
}
impl From<Socket> for std::os::fd::OwnedFd {
    fn from(s: Socket) -> std::os::fd::OwnedFd { match s.never {} }
}

/// A borrowed socket; it can never be made from a real stream in a browser tab.
pub struct SockRef<'s> {
    socket: &'s Socket,
}
impl<'s> std::ops::Deref for SockRef<'s> {
    type Target = Socket;
    fn deref(&self) -> &Socket { self.socket }
}
impl<'s> fmt::Debug for SockRef<'s> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.socket.fmt(f) }
}
impl<'s, S> From<&'s S> for SockRef<'s>
where
    S: std::os::fd::AsFd,
{
    fn from(_: &'s S) -> Self { panic!("no sockets in a browser tab") }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterfaceIndexOrAddress {
    Index(u32),
    Address(Ipv4Addr),
}

pub struct MaybeUninitSlice<'a>(&'a mut [MaybeUninit<u8>]);
impl<'a> MaybeUninitSlice<'a> {
    pub fn new(buf: &'a mut [MaybeUninit<u8>]) -> MaybeUninitSlice<'a> { MaybeUninitSlice(buf) }
}
impl<'a> std::ops::Deref for MaybeUninitSlice<'a> {
    type Target = [MaybeUninit<u8>];
    fn deref(&self) -> &[MaybeUninit<u8>] { self.0 }
}
impl<'a> std::ops::DerefMut for MaybeUninitSlice<'a> {
    fn deref_mut(&mut self) -> &mut [MaybeUninit<u8>] { self.0 }
}
