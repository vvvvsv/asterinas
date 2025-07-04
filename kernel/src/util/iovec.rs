// SPDX-License-Identifier: MPL-2.0

use ostd::mm::{Infallible, VmSpace};

use crate::prelude::*;

/// A kernel space IO vector.
#[derive(Debug, Clone, Copy)]
struct IoVec {
    base: Vaddr,
    len: usize,
}

/// A user space IO vector.
///
/// The difference between `IoVec` and `UserIoVec`
/// is that `UserIoVec` uses `isize` as the length type,
/// while `IoVec` uses `usize`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod)]
struct UserIoVec {
    base: Vaddr,
    len: isize,
}

impl TryFrom<UserIoVec> for IoVec {
    type Error = Error;

    fn try_from(value: UserIoVec) -> Result<Self> {
        if value.len < 0 {
            return_errno_with_message!(Errno::EINVAL, "the length of IO vector cannot be negative");
        }

        Ok(IoVec {
            base: value.base,
            len: value.len as usize,
        })
    }
}

impl IoVec {
    /// Returns whether the `IoVec` points to an empty user buffer.
    const fn is_empty(&self) -> bool {
        self.len == 0 || self.base == 0
    }

    fn reader<'a>(&self, vm_space: &'a VmSpace) -> Result<VmReader<'a>> {
        Ok(vm_space.reader(self.base, self.len)?)
    }

    fn writer<'a>(&self, vm_space: &'a VmSpace) -> Result<VmWriter<'a>> {
        Ok(vm_space.writer(self.base, self.len)?)
    }
}

/// The util function for create [`VmReader`]/[`VmWriter`]s.
fn copy_iovs_and_convert<'a, T: 'a>(
    user_space: &'a CurrentUserSpace<'a>,
    start_addr: Vaddr,
    count: usize,
    convert_iovec: impl Fn(&IoVec, &'a VmSpace) -> Result<T>,
) -> Result<Box<[T]>> {
    let vm_space = user_space.root_vmar().vm_space();

    let mut v = Vec::with_capacity(count);
    for idx in 0..count {
        let iov = {
            let addr = start_addr + idx * core::mem::size_of::<UserIoVec>();
            let uiov: UserIoVec = vm_space
                .reader(addr, core::mem::size_of::<UserIoVec>())?
                .read_val()?;
            IoVec::try_from(uiov)?
        };

        if iov.is_empty() {
            continue;
        }

        let converted = convert_iovec(&iov, vm_space)?;
        v.push(converted)
    }

    Ok(v.into_boxed_slice())
}

/// A collection of [`VmReader`]s.
///
/// Such readers are built from user-provided buffer, so it's always fallible.
pub struct VmReaderArray<'a>(Box<[VmReader<'a>]>);

/// A collection of [`VmWriter`]s.
///
/// Such writers are built from user-provided buffer, so it's always fallible.
pub struct VmWriterArray<'a>(Box<[VmWriter<'a>]>);

impl<'a> VmReaderArray<'a> {
    /// Creates a new `VmReaderArray` from user-provided io vec buffer.
    pub fn from_user_io_vecs(
        user_space: &'a CurrentUserSpace<'a>,
        start_addr: Vaddr,
        count: usize,
    ) -> Result<Self> {
        let readers = copy_iovs_and_convert(user_space, start_addr, count, IoVec::reader)?;
        Ok(Self(readers))
    }

    /// Returns mutable reference to [`VmReader`]s.
    pub fn readers_mut(&mut self) -> &mut [VmReader<'a>] {
        &mut self.0
    }

    /// Creates a new `VmReaderArray`.
    #[cfg(ktest)]
    pub const fn new(readers: Box<[VmReader<'a>]>) -> Self {
        Self(readers)
    }
}

impl<'a> VmWriterArray<'a> {
    /// Creates a new `VmWriterArray` from user-provided io vec buffer.
    pub fn from_user_io_vecs(
        user_space: &'a CurrentUserSpace<'a>,
        start_addr: Vaddr,
        count: usize,
    ) -> Result<Self> {
        let writers = copy_iovs_and_convert(user_space, start_addr, count, IoVec::writer)?;
        Ok(Self(writers))
    }

    /// Returns mutable reference to [`VmWriter`]s.
    pub fn writers_mut(&mut self) -> &mut [VmWriter<'a>] {
        &mut self.0
    }
}

/// Trait defining the read behavior for a collection of [`VmReader`]s.
pub trait MultiRead: ReadCString {
    /// Reads the exact number of bytes required to exhaust `self` or fill `writer`,
    /// accumulating total bytes read.
    ///
    /// If the return value is `Ok(n)`,
    /// then `n` should be `min(self.sum_lens(), writer.avail())`.
    ///
    /// # Errors
    ///
    /// This method returns [`Errno::EFAULT`] if a page fault occurs.
    /// The position of `self` and the `writer` is left unspecified when this method returns error.
    fn read(&mut self, writer: &mut VmWriter<'_, Infallible>) -> Result<usize>;

    /// Calculates the total length of data remaining to read.
    fn sum_lens(&self) -> usize;

    /// Checks if the data remaining to read is empty.
    fn is_empty(&self) -> bool {
        self.sum_lens() == 0
    }

    /// Skips the first `nbytes` bytes of data, or skips to the end if the readers have
    /// insufficient bytes.
    fn skip_some(&mut self, nbytes: usize);
}

/// Trait defining the write behavior for a collection of [`VmWriter`]s.
pub trait MultiWrite {
    /// Writes the exact number of bytes required to exhaust `writer` or fill `self`,
    /// accumulating total bytes read.
    ///
    /// If the return value is `Ok(n)`,
    /// then `n` should be `min(self.sum_lens(), reader.remain())`.
    ///
    /// # Errors
    ///
    /// This method returns [`Errno::EFAULT`] if a page fault occurs.
    /// The position of `self` and the `reader` is left unspecified when this method returns error.
    fn write(&mut self, reader: &mut VmReader<'_, Infallible>) -> Result<usize>;

    /// Calculates the length of space available to write.
    fn sum_lens(&self) -> usize;

    /// Checks if the space available to write is empty.
    fn is_empty(&self) -> bool {
        self.sum_lens() == 0
    }

    /// Skips the first `nbytes` bytes of data, or skips to the end if the writers have
    /// insufficient bytes.
    fn skip_some(&mut self, nbytes: usize);
}

impl MultiRead for VmReaderArray<'_> {
    fn read(&mut self, writer: &mut VmWriter<'_, Infallible>) -> Result<usize> {
        let mut total_len = 0;

        for reader in &mut self.0 {
            let copied_len = reader.read_fallible(writer)?;
            total_len += copied_len;
            if !writer.has_avail() {
                break;
            }
        }
        Ok(total_len)
    }

    fn sum_lens(&self) -> usize {
        self.0.iter().map(|vm_reader| vm_reader.remain()).sum()
    }

    fn skip_some(&mut self, mut nbytes: usize) {
        for reader in &mut self.0 {
            let bytes_to_skip = reader.remain().min(nbytes);
            reader.skip(bytes_to_skip);
            nbytes -= bytes_to_skip;

            if nbytes == 0 {
                return;
            }
        }
    }
}

impl MultiRead for VmReader<'_> {
    fn read(&mut self, writer: &mut VmWriter<'_, Infallible>) -> Result<usize> {
        Ok(self.read_fallible(writer)?)
    }

    fn sum_lens(&self) -> usize {
        self.remain()
    }

    fn skip_some(&mut self, nbytes: usize) {
        self.skip(self.remain().min(nbytes));
    }
}

impl dyn MultiRead + '_ {
    /// Reads a `T` value, returning a `None` if the readers have insufficient bytes.
    pub fn read_val_opt<T: Pod>(&mut self) -> Result<Option<T>> {
        let mut val = T::new_zeroed();
        let nbytes = self.read(&mut VmWriter::from(val.as_bytes_mut()))?;

        if nbytes == size_of::<T>() {
            Ok(Some(val))
        } else {
            Ok(None)
        }
    }
}

impl MultiWrite for VmWriterArray<'_> {
    fn write(&mut self, reader: &mut VmReader<'_, Infallible>) -> Result<usize> {
        let mut total_len = 0;

        for writer in &mut self.0 {
            let copied_len = writer.write_fallible(reader)?;
            total_len += copied_len;
            if !reader.has_remain() {
                break;
            }
        }
        Ok(total_len)
    }

    fn sum_lens(&self) -> usize {
        self.0.iter().map(|vm_writer| vm_writer.avail()).sum()
    }

    fn skip_some(&mut self, mut nbytes: usize) {
        for writer in &mut self.0 {
            let bytes_to_skip = writer.avail().min(nbytes);
            writer.skip(bytes_to_skip);
            nbytes -= bytes_to_skip;

            if nbytes == 0 {
                return;
            }
        }
    }
}

impl MultiWrite for VmWriter<'_> {
    fn write(&mut self, reader: &mut VmReader<'_, Infallible>) -> Result<usize> {
        Ok(self.write_fallible(reader)?)
    }

    fn sum_lens(&self) -> usize {
        self.avail()
    }

    fn skip_some(&mut self, nbytes: usize) {
        self.skip(self.avail().min(nbytes));
    }
}

impl dyn MultiWrite + '_ {
    /// Writes a `T` value, truncating the value if the writers have insufficient bytes.
    pub fn write_val_trunc<T: Pod>(&mut self, val: &T) -> Result<()> {
        let _nbytes = self.write(&mut VmReader::from(val.as_bytes()))?;
        // `_nbytes` may be smaller than the value size. We ignore it to truncate the value.

        Ok(())
    }
}
