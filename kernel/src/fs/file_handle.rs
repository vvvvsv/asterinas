// SPDX-License-Identifier: MPL-2.0

#![expect(unused_variables)]

//! Opened File Handle

use core::time::Duration;

use aster_rights::Full;

use super::inode_handle::InodeHandle;
use crate::{
    events::IoEvents,
    fs::utils::{
        AccessMode, FallocMode, FileSystem, InodeMode, InodeType, IoctlCmd, Metadata, SeekFrom,
        StatusFlags, XattrName, XattrNamespace, XattrSetFlags,
    },
    net::socket::Socket,
    prelude::*,
    process::{
        signal::{PollHandle, Pollable},
        Gid, Uid,
    },
    vm::vmo::Vmo,
};

/// The basic operations defined on a file
pub trait FileLike: Pollable + Send + Sync + Any {
    fn read(&self, writer: &mut VmWriter) -> Result<usize> {
        return_errno_with_message!(Errno::EBADF, "the file is not valid for reading");
    }

    fn write(&self, reader: &mut VmReader) -> Result<usize> {
        return_errno_with_message!(Errno::EBADF, "the file is not valid for writing");
    }

    /// Read at the given file offset.
    ///
    /// The file must be seekable to support `read_at`.
    /// Unlike [`read`], `read_at` will not change the file offset.
    ///
    /// [`read`]: FileLike::read
    fn read_at(&self, offset: usize, writer: &mut VmWriter) -> Result<usize> {
        return_errno_with_message!(Errno::ESPIPE, "read_at is not supported");
    }

    /// Write at the given file offset.
    ///
    /// The file must be seekable to support `write_at`.
    /// Unlike [`write`], `write_at` will not change the file offset.
    /// If the file is append-only, the `offset` will be ignored.
    ///
    /// [`write`]: FileLike::write
    fn write_at(&self, offset: usize, reader: &mut VmReader) -> Result<usize> {
        return_errno_with_message!(Errno::ESPIPE, "write_at is not supported");
    }

    fn ioctl(&self, cmd: IoctlCmd, arg: usize) -> Result<i32> {
        return_errno_with_message!(Errno::EINVAL, "ioctl is not supported");
    }

    fn resize(&self, new_size: usize) -> Result<()> {
        return_errno_with_message!(Errno::EINVAL, "resize is not supported");
    }

    /// Get the metadata that describes this file.
    fn metadata(&self) -> Metadata;

    fn mode(&self) -> Result<InodeMode> {
        return_errno_with_message!(Errno::EINVAL, "mode is not supported");
    }

    fn set_mode(&self, mode: InodeMode) -> Result<()> {
        return_errno_with_message!(Errno::EINVAL, "set_mode is not supported");
    }

    fn owner(&self) -> Result<Uid> {
        return_errno_with_message!(Errno::EPERM, "owner is not supported");
    }

    fn set_owner(&self, uid: Uid) -> Result<()> {
        return_errno_with_message!(Errno::EPERM, "set_owner is not supported");
    }

    fn group(&self) -> Result<Gid> {
        return_errno_with_message!(Errno::EPERM, "group is not supported");
    }

    fn set_group(&self, gid: Gid) -> Result<()> {
        return_errno_with_message!(Errno::EPERM, "set_group is not supported");
    }

    fn status_flags(&self) -> StatusFlags {
        StatusFlags::empty()
    }

    fn set_status_flags(&self, _new_flags: StatusFlags) -> Result<()> {
        return_errno_with_message!(Errno::EINVAL, "set_status_flags is not supported");
    }

    fn access_mode(&self) -> AccessMode {
        AccessMode::O_RDWR
    }

    fn seek(&self, seek_from: SeekFrom) -> Result<usize> {
        return_errno_with_message!(Errno::ESPIPE, "seek is not supported");
    }

    fn fallocate(&self, mode: FallocMode, offset: usize, len: usize) -> Result<()> {
        return_errno_with_message!(Errno::EOPNOTSUPP, "fallocate is not supported");
    }

    fn as_socket(&self) -> Option<&dyn Socket> {
        None
    }
}

impl dyn FileLike {
    pub fn downcast_ref<T: FileLike>(&self) -> Option<&T> {
        (self as &dyn Any).downcast_ref::<T>()
    }

    pub fn read_bytes(&self, buf: &mut [u8]) -> Result<usize> {
        let mut writer = VmWriter::from(buf).to_fallible();
        self.read(&mut writer)
    }

    pub fn write_bytes(&self, buf: &[u8]) -> Result<usize> {
        let mut reader = VmReader::from(buf).to_fallible();
        self.write(&mut reader)
    }

    pub fn read_bytes_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        let mut writer = VmWriter::from(buf).to_fallible();
        self.read_at(offset, &mut writer)
    }

    pub fn write_bytes_at(&self, offset: usize, buf: &[u8]) -> Result<usize> {
        let mut reader = VmReader::from(buf).to_fallible();
        self.write_at(offset, &mut reader)
    }

    pub fn as_socket_or_err(&self) -> Result<&dyn Socket> {
        self.as_socket()
            .ok_or_else(|| Error::with_message(Errno::ENOTSOCK, "the file is not a socket"))
    }

    pub fn as_inode_or_err(&self) -> Result<&InodeHandle> {
        self.downcast_ref().ok_or_else(|| {
            Error::with_message(Errno::EINVAL, "the file is not related to an inode")
        })
    }
}

/// Any type that may be referred to by a file descriptor, behaving like an inode.
///
/// In UNIX, everything is a file. In Linux, everything is an inode:
/// even the likes of sockets, pipes, and epoll files are backed by (dummy) inodes.
/// This is why Linux has sockfs, pipefs, and anon_inode_fs.
/// To mimic Linux's "everything is an inode" behavior,
/// the `InodeLike` trait inherits many--but not all--methods from the `Inode` trait.
/// We abandon the methods that will never be invoked on "fake inodes".
///
/// In addition to the inode interface,
/// `InodeLike` provides conversion methods to
/// downcast a trait object of `InodeLike` to a socket trait object with `as_socket`
/// and to a concrete type of `T: InodeLike` with `downcast_ref`.
/// Real inode-backed files are represented by `InodeHandle`.
/// So a convenient `as_inode` method is provided to obtain a reference to `DentryHandle`.
pub trait InodeLike: Pollable + Any + Sync + Send {
    /************************** Related to metadata **************************/

    /// Get the metadata that describes this file.
    fn metadata(&self) -> Metadata;

    fn type_(&self) -> InodeType;

    fn ino(&self) -> u64;

    fn mode(&self) -> Result<InodeMode> {
        return_errno_with_message!(Errno::EINVAL, "mode is not supported");
    }

    fn set_mode(&self, mode: InodeMode) -> Result<()> {
        return_errno_with_message!(Errno::EINVAL, "set_mode is not supported");
    }

    fn owner(&self) -> Result<Uid> {
        return_errno_with_message!(Errno::EPERM, "owner is not supported");
    }

    fn set_owner(&self, uid: Uid) -> Result<()> {
        return_errno_with_message!(Errno::EPERM, "set_owner is not supported");
    }

    fn group(&self) -> Result<Gid> {
        return_errno_with_message!(Errno::EPERM, "group is not supported");
    }

    fn set_group(&self, gid: Gid) -> Result<()> {
        return_errno_with_message!(Errno::EPERM, "set_group is not supported");
    }

    fn atime(&self) -> Duration {
        Duration::from_secs(0)
    }

    fn set_atime(&self, time: Duration) {}

    fn mtime(&self) -> Duration {
        Duration::from_secs(0)
    }

    fn set_mtime(&self, time: Duration) {}

    fn ctime(&self) -> Duration {
        Duration::from_secs(0)
    }

    fn set_ctime(&self, time: Duration) {}

    fn status_flags(&self) -> StatusFlags {
        StatusFlags::empty()
    }

    fn set_status_flags(&self, _new_flags: StatusFlags) -> Result<()> {
        return_errno_with_message!(Errno::EINVAL, "set_status_flags is not supported");
    }

    fn access_mode(&self) -> AccessMode {
        AccessMode::O_RDWR
    }

    /// Returns a user-friendly display name.
    ///
    /// One use of this method is to provide the content of
    /// entries under `/proc/<pid>/fd/<fd>`.
    /// Thus, to be compatible with Linux's behavior,
    /// this method returns a string that follows a particular format:
    /// * For regular files backed by real inodes,
    /// the display name is the absolute path of the inode.
    /// * For sockets and pipes, the display name usually follows the format
    /// of `file_type:[inode_num]` (e.g., `socket:123456`).
    /// * For other special files (such as epoll, eventfd, and timerfd),
    /// the display name is in the format of `anon_inode:[file_type]`
    /// (e.g., `anon_inode:[eventpoll]`).
    fn display_name(&self) -> String;

    fn fs(&self) -> Arc<dyn FileSystem>;

    /**************************** Related to I/O *****************************/

    fn read(&self, writer: &mut VmWriter) -> Result<usize> {
        return_errno_with_message!(Errno::EBADF, "the file is not valid for reading");
    }

    fn write(&self, reader: &mut VmReader) -> Result<usize> {
        return_errno_with_message!(Errno::EBADF, "the file is not valid for writing");
    }

    /// Read at the given file offset.
    ///
    /// The file must be seekable to support `read_at`.
    /// Unlike [`read`], `read_at` will not change the file offset.
    ///
    /// [`read`]: InodeLike::read
    fn read_at(&self, offset: usize, writer: &mut VmWriter) -> Result<usize> {
        return_errno_with_message!(Errno::ESPIPE, "read_at is not supported");
    }

    /// Write at the given file offset.
    ///
    /// The file must be seekable to support `write_at`.
    /// Unlike [`write`], `write_at` will not change the file offset.
    /// If the file is append-only, the `offset` will be ignored.
    ///
    /// [`write`]: InodeLike::write
    fn write_at(&self, offset: usize, reader: &mut VmReader) -> Result<usize> {
        return_errno_with_message!(Errno::ESPIPE, "write_at is not supported");
    }

    fn ioctl(&self, cmd: IoctlCmd, arg: usize) -> Result<i32> {
        return_errno_with_message!(Errno::EINVAL, "ioctl is not supported");
    }

    fn sync_all(&self) -> Result<()> {
        Ok(())
    }

    fn sync_data(&self) -> Result<()> {
        Ok(())
    }

    fn poll(&self, mask: IoEvents, _poller: Option<&mut PollHandle>) -> IoEvents {
        let events = IoEvents::IN | IoEvents::OUT;
        events & mask
    }

    fn page_cache(&self) -> Option<Vmo<Full>> {
        None
    }

    fn seek(&self, seek_from: SeekFrom) -> Result<usize> {
        return_errno_with_message!(Errno::ESPIPE, "seek is not supported");
    }

    /******************** Related to file space and size *********************/

    fn size(&self) -> usize {
        0
    }

    fn resize(&self, new_size: usize) -> Result<()> {
        return_errno_with_message!(Errno::EINVAL, "resize is not supported");
    }

    /// Manipulates a range of space of the file according to the specified allocate mode,
    /// the manipulated range starts at `offset` and continues for `len` bytes.
    fn fallocate(&self, mode: FallocMode, offset: usize, len: usize) -> Result<()> {
        return_errno_with_message!(Errno::EOPNOTSUPP, "fallocate is not supported");
    }

    /******************** Related to extended attributes *********************/

    fn set_xattr(
        &self,
        name: XattrName,
        value_reader: &mut VmReader,
        flags: XattrSetFlags,
    ) -> Result<()> {
        Err(Error::new(Errno::EOPNOTSUPP))
    }

    fn get_xattr(&self, name: XattrName, value_writer: &mut VmWriter) -> Result<usize> {
        Err(Error::new(Errno::EOPNOTSUPP))
    }

    fn list_xattr(&self, namespace: XattrNamespace, list_writer: &mut VmWriter) -> Result<usize> {
        Err(Error::new(Errno::EOPNOTSUPP))
    }

    fn remove_xattr(&self, name: XattrName) -> Result<()> {
        Err(Error::new(Errno::EOPNOTSUPP))
    }

    /********************** Downcasting to a sub-trait ***********************/

    fn as_socket(&self) -> Option<&dyn Socket> {
        None
    }
}

impl dyn InodeLike {
    pub fn downcast_ref<T: InodeLike>(&self) -> Option<&T> {
        (self as &dyn Any).downcast_ref::<T>()
    }

    pub fn read_bytes(&self, buf: &mut [u8]) -> Result<usize> {
        let mut writer = VmWriter::from(buf).to_fallible();
        self.read(&mut writer)
    }

    pub fn write_bytes(&self, buf: &[u8]) -> Result<usize> {
        let mut reader = VmReader::from(buf).to_fallible();
        self.write(&mut reader)
    }

    pub fn read_bytes_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        let mut writer = VmWriter::from(buf).to_fallible();
        self.read_at(offset, &mut writer)
    }

    pub fn write_bytes_at(&self, offset: usize, buf: &[u8]) -> Result<usize> {
        let mut reader = VmReader::from(buf).to_fallible();
        self.write_at(offset, &mut reader)
    }

    pub fn as_socket_or_err(&self) -> Result<&dyn Socket> {
        self.as_socket()
            .ok_or_else(|| Error::with_message(Errno::ENOTSOCK, "the file is not a socket"))
    }

    // pub fn as_inode_or_err(&self) -> Result<&InodeHandle> {
    //     self.downcast_ref().ok_or_else(|| {
    //         Error::with_message(Errno::EINVAL, "the file is not related to an inode")
    //     })
    // }
}
