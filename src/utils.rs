use std::mem::MaybeUninit;
use std::os::unix::prelude::RawFd;

pub const F_PUNCHHOLE: i32 = 99;

pub fn get_fs_block_size(fd: RawFd) -> u64 {
    let mut stat = MaybeUninit::uninit();
    let ret = unsafe { libc::fstatvfs(fd, stat.as_mut_ptr()) };

    if ret != 0 {
        panic!("fstat failed");
    }
    let stat = unsafe { stat.assume_init() };
    stat.f_bsize as u64
}

pub fn seek_data(fd: RawFd, offset: u64) -> u64 {
    let ret = unsafe { libc::lseek(fd, offset.try_into().unwrap(), libc::SEEK_DATA) };
    if ret == -1 {
        panic!("lseek failed");
    }

    ret as u64
}

pub fn is_zeroed(buf: &[u8]) -> bool {
    let (prefix, aligned, suffix) = unsafe { buf.align_to::<usize>() };

    prefix.iter().all(|&x| x == 0)
        && aligned.iter().all(|&x| x == 0)
        && suffix.iter().all(|&x| x == 0)
}

#[cfg(target_os = "macos")]
#[repr(C)]
pub struct PunchHoleArgs {
    _fp_flags: libc::c_uint,
    _reserved: libc::c_uint,
    fp_offset: libc::off_t,
    fp_length: libc::off_t,
}

impl PunchHoleArgs {
    pub fn new(offset: u64, length: u64) -> Self {
        Self {
            _fp_flags: 0,
            _reserved: 0,
            fp_offset: offset.try_into().unwrap(),
            fp_length: length.try_into().unwrap(),
        }
    }
}
