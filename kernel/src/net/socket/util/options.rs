// SPDX-License-Identifier: MPL-2.0

use core::{ops::RangeInclusive, time::Duration};

use aster_bigtcp::socket::{
    NeedIfacePoll, TCP_RECV_BUF_LEN, TCP_SEND_BUF_LEN, UDP_RECV_PAYLOAD_LEN, UDP_SEND_PAYLOAD_LEN,
};

use crate::{
    current_userspace, match_sock_option_mut, match_sock_option_ref,
    net::socket::{
        options::{
            AcceptConn, AttachFilter, KeepAlive, Linger, PassCred, PeerCred, Priority, RecvBuf,
            RecvBufForce, ReuseAddr, ReusePort, SendBuf, SendBufForce, SocketOption,
        },
        unix::{CUserCred, UNIX_STREAM_DEFAULT_BUF_SIZE},
    },
    prelude::*,
    process::{credentials::capabilities::CapSet, posix_thread::AsPosixThread},
};

#[derive(Debug, Clone, CopyGetters, Setters)]
#[get_copy = "pub"]
#[set = "pub"]
pub struct SocketOptionSet {
    reuse_addr: bool,
    reuse_port: bool,
    send_buf: u32,
    recv_buf: u32,
    linger: LingerOption,
    keep_alive: bool,
    priority: i32,
    pass_cred: bool,
    #[getset(skip)]
    #[getset(set)]
    attach_filter: Option<FilterProgram>,
}

impl Default for SocketOptionSet {
    fn default() -> Self {
        Self {
            reuse_addr: false,
            reuse_port: false,
            send_buf: MIN_SENDBUF,
            recv_buf: MIN_RECVBUF,
            linger: LingerOption::default(),
            keep_alive: false,
            priority: 0,
            pass_cred: false,
            attach_filter: None,
        }
    }
}

impl SocketOptionSet {
    /// Return the default socket level options for tcp socket.
    pub fn new_tcp() -> Self {
        Self {
            send_buf: TCP_SEND_BUF_LEN as u32,
            recv_buf: TCP_RECV_BUF_LEN as u32,
            ..Default::default()
        }
    }

    /// Return the default socket level options for udp socket.
    pub fn new_udp() -> Self {
        Self {
            send_buf: UDP_SEND_PAYLOAD_LEN as u32,
            recv_buf: UDP_RECV_PAYLOAD_LEN as u32,
            ..Default::default()
        }
    }

    /// Returns the default socket level options for unix stream socket.
    pub(in crate::net) fn new_unix_stream() -> Self {
        Self {
            send_buf: UNIX_STREAM_DEFAULT_BUF_SIZE as u32,
            recv_buf: UNIX_STREAM_DEFAULT_BUF_SIZE as u32,
            ..Default::default()
        }
    }

    /// Gets socket-level options.
    ///
    /// Note that the socket error has to be handled separately, because it is automatically
    /// cleared after reading. This method does not handle it. Instead,
    /// [`Self::get_and_clear_socket_errors`] should be used.
    pub fn get_option(
        &self,
        option: &mut dyn SocketOption,
        socket: &dyn GetSocketLevelOption,
    ) -> Result<()> {
        match_sock_option_mut!(option, {
            socket_reuse_addr: ReuseAddr => {
                let reuse_addr = self.reuse_addr();
                socket_reuse_addr.set(reuse_addr);
            },
            socket_send_buf: SendBuf => {
                let send_buf = self.send_buf();
                socket_send_buf.set(send_buf);
            },
            socket_recv_buf: RecvBuf => {
                let recv_buf = self.recv_buf();
                socket_recv_buf.set(recv_buf);
            },
            socket_reuse_port: ReusePort => {
                let reuse_port = self.reuse_port();
                socket_reuse_port.set(reuse_port);
            },
            socket_linger: Linger => {
                let linger = self.linger();
                socket_linger.set(linger);
            },
            socket_priority: Priority => {
                let priority = self.priority();
                socket_priority.set(priority);
            },
            socket_keepalive: KeepAlive => {
                let keep_alive = self.keep_alive();
                socket_keepalive.set(keep_alive);
            },
            socket_pass_cred: PassCred => {
                // FIXME: This option is unix socket only.
                // Should we return errors if the socket is not unix socket?
                let pass_cred = self.pass_cred();
                socket_pass_cred.set(pass_cred);
            },
            socket_peer_cred: PeerCred => {
                let peer_cred = socket.peer_cred()?;
                socket_peer_cred.set(peer_cred);
            },
            socket_accept_conn: AcceptConn => {
                let is_listening = socket.is_listening();
                socket_accept_conn.set(is_listening);
            },
            socket_sendbuf_force: SendBufForce => {
                check_current_privileged()?;
                let send_buf = self.send_buf();
                socket_sendbuf_force.set(send_buf);
            },
            socket_recvbuf_force: RecvBufForce => {
                check_current_privileged()?;
                let recv_buf = self.recv_buf();
                socket_recvbuf_force.set(recv_buf);
            },
            _ => return_errno_with_message!(Errno::ENOPROTOOPT, "the socket option to get is unknown")
        });
        Ok(())
    }

    /// Sets socket-level options.
    pub fn set_option(
        &mut self,
        option: &dyn SocketOption,
        socket: &dyn SetSocketLevelOption,
    ) -> Result<NeedIfacePoll> {
        match_sock_option_ref!(option, {
            socket_send_buf: SendBuf => {
                let send_buf = socket_send_buf.get().unwrap();
                if *send_buf <= MIN_SENDBUF {
                    self.set_send_buf(MIN_SENDBUF);
                } else {
                    self.set_send_buf(*send_buf);
                }
            },
            socket_recv_buf: RecvBuf => {
                let recv_buf = socket_recv_buf.get().unwrap();
                if *recv_buf <= MIN_RECVBUF {
                    self.set_recv_buf(MIN_RECVBUF);
                } else {
                    self.set_recv_buf(*recv_buf);
                }
            },
            socket_reuse_addr: ReuseAddr => {
                let reuse_addr = socket_reuse_addr.get().unwrap();
                self.set_reuse_addr(*reuse_addr);
            },
            socket_reuse_port: ReusePort => {
                let reuse_port = socket_reuse_port.get().unwrap();
                self.set_reuse_port(*reuse_port);
            },
            socket_priority: Priority => {
                let priority = socket_priority.get().unwrap();
                check_priority(*priority)?;
                self.set_priority(*priority);
            },
            socket_linger: Linger => {
                let linger = socket_linger.get().unwrap();
                self.set_linger(*linger);
            },
            socket_keepalive: KeepAlive => {
                let keep_alive = socket_keepalive.get().unwrap();
                self.set_keep_alive(*keep_alive);
                return Ok(socket.set_keep_alive(*keep_alive));
            },
            socket_pass_cred: PassCred => {
                // FIXME: This option is unix socket only.
                // Should we return errors if the socket is not unix socket?
                let pass_cred = socket_pass_cred.get().unwrap();
                self.set_pass_cred(*pass_cred);
            },
            socket_attach_filter: AttachFilter => {
                let attach_filter = socket_attach_filter.get().unwrap();
                self.set_attach_filter(Some(attach_filter.clone()));
            },
            socket_sendbuf_force: SendBufForce => {
                check_current_privileged()?;
                let send_buf = socket_sendbuf_force.get().unwrap();
                if *send_buf <= MIN_SENDBUF {
                    self.set_send_buf(MIN_SENDBUF);
                } else {
                    self.set_send_buf(*send_buf);
                }
            },
            socket_recvbuf_force: RecvBufForce => {
                check_current_privileged()?;
                let recv_buf = socket_recvbuf_force.get().unwrap();
                if *recv_buf <= MIN_RECVBUF {
                    self.set_recv_buf(MIN_RECVBUF);
                } else {
                    self.set_recv_buf(*recv_buf);
                }
            },
            _ => return_errno_with_message!(Errno::ENOPROTOOPT, "the socket option to be set is unknown")
        });

        Ok(NeedIfacePoll::FALSE)
    }
}

fn check_current_privileged() -> Result<()> {
    let credentials = {
        let current = current_thread!();
        let posix_thread = current.as_posix_thread().unwrap();
        posix_thread.credentials()
    };

    if credentials.euid().is_root() || credentials.effective_capset().contains(CapSet::NET_ADMIN) {
        return Ok(());
    }

    return_errno_with_message!(Errno::EPERM, "the process does not have permissions")
}

fn check_priority(priority: i32) -> Result<()> {
    const NORMAL_PRIORITY_RANGE: RangeInclusive<i32> = 0..=6;

    if NORMAL_PRIORITY_RANGE.contains(&priority) {
        return Ok(());
    }

    check_current_privileged()
}

pub const MIN_SENDBUF: u32 = 2304;
pub const MIN_RECVBUF: u32 = 2304;

#[derive(Debug, Default, Clone, Copy)]
pub struct LingerOption {
    is_on: bool,
    timeout: Duration,
}

impl LingerOption {
    pub fn new(is_on: bool, timeout: Duration) -> Self {
        Self { is_on, timeout }
    }

    pub fn is_on(&self) -> bool {
        self.is_on
    }

    pub fn timeout(&self) -> Duration {
        self.timeout
    }
}

/// Reference: <https://elixir.bootlin.com/linux/v6.0.9/source/include/uapi/linux/filter.h#L24>.
// FIXME: We should define suitable Rust type instead of using the C type inside `FilterProgram`.
#[derive(Clone, Copy, Debug, Pod)]
#[repr(C)]
struct CSockFilter {
    code: u16,
    jt: u8,
    jf: u8,
    k: u32,
}

#[expect(dead_code)]
#[derive(Debug, Clone)]
pub struct FilterProgram(Arc<[CSockFilter]>);

impl FilterProgram {
    pub fn read_from_user(addr: Vaddr, count: usize) -> Result<Self> {
        let mut filters = Vec::with_capacity(count);

        for i in 0..count {
            let addr = addr + i * core::mem::size_of::<CSockFilter>();
            let sock_filter = current_userspace!().read_val::<CSockFilter>(addr)?;
            filters.push(sock_filter);
        }

        Ok(Self(filters.into()))
    }
}

pub(in crate::net) trait GetSocketLevelOption {
    /// Returns whether the socket is in listening state.
    fn is_listening(&self) -> bool;
    /// Returns the peer credentials.
    fn peer_cred(&self) -> Result<CUserCred> {
        return_errno_with_message!(Errno::ENOPROTOOPT, "SO_PEERCRED is not supported")
    }
}

/// A trait used for setting socket level options on actual sockets.
pub(in crate::net) trait SetSocketLevelOption {
    /// Sets whether keepalive messages are enabled.
    fn set_keep_alive(&self, _keep_alive: bool) -> NeedIfacePoll {
        NeedIfacePoll::FALSE
    }
}
