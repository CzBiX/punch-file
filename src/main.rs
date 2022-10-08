use std::cmp;
use std::fs::File;
use std::io::Read;
use std::os::unix::prelude::{AsRawFd, RawFd};
use std::path::PathBuf;

use clap::Parser;
use clap_num::maybe_hex;
use utils::*;

mod utils;

#[derive(Parser, Debug)]
struct Args {
    file: PathBuf,

    #[arg(short, long)]
    verbose: bool,

    #[arg(long, value_parser = maybe_hex::<u64>, help = "Start offset in file")]
    offset: Option<u64>,

    #[arg(long, value_parser = maybe_hex::<u64>, help = "Limit bytes to check")]
    length: Option<u64>,

    #[arg(long, help = "Don't actually punch holes")]
    dry_run: bool,
}

struct HoleInfo {
    offset: u64,
    length: u64,
}

struct Main {
    file: PathBuf,
    verbose: bool,
    offset: u64,
    max_offset: Option<u64>,
    dry_run: bool,
}

impl Main {
    fn mark_hole(&self, fd: RawFd, hole: &HoleInfo) {
        if self.verbose {
            println!(
                "hole: offset: {:#X}, length: {:#X}",
                hole.offset, hole.length
            );
        }

        if self.dry_run {
            return;
        }

        let ret = unsafe {
            libc::fcntl(
                fd,
                F_PUNCHHOLE,
                PunchHoleArgs::new(hole.offset, hole.length),
            )
        };

        if ret == -1 {
            panic!("punch hole failed");
        }
    }

    fn process(&self) {
        let mut file = File::options()
            .read(true)
            .write(true)
            .open(&self.file)
            .unwrap();

        let block_size = {
            let block_size = get_fs_block_size(file.as_raw_fd());
            block_size.try_into().unwrap()
        };

        if self.verbose {
            println!("Filesytem block size: {:#X}", block_size);
        }

        let mut buf = vec![0u8; block_size];
        let mut last_hole = None::<HoleInfo>;

        let mut offset = self.offset;
        let max_offset = {
            let file_size = file.metadata().unwrap().len();

            self.max_offset
                .map_or(file_size, |max_offset| cmp::min(file_size, max_offset))
        };

        loop {
            if offset >= max_offset {
                break;
            }

            let new_offset = match seek_data(file.as_raw_fd(), offset) {
                Some(new_offset) => new_offset,
                None => {
                    break;
                }
            };

            if new_offset != offset {
                if self.verbose {
                    println!(
                        "skip: offset: {:#X}, length: {:#X}",
                        offset,
                        new_offset - offset
                    );
                }

                offset = new_offset;
            }
            if new_offset >= max_offset {
                break;
            }

            let read_count = file.read(&mut buf).unwrap();
            if read_count == 0 {
                panic!("EOF");
            }

            if is_zeroed(&buf) {
                match &mut last_hole {
                    Some(hole) => {
                        hole.length = hole.length.checked_add(read_count as u64).unwrap();
                    }
                    None => {
                        last_hole = Some(HoleInfo {
                            offset,
                            length: block_size as u64,
                        });
                    }
                }
            } else {
                if let Some(hole) = last_hole {
                    self.mark_hole(file.as_raw_fd(), &hole);
                    last_hole = None;
                }

                if self.verbose {
                    println!("data: offset: {:#X}", offset);
                }
            }

            offset = offset.checked_add(read_count as u64).unwrap();
        }

        if let Some(hole) = last_hole {
            self.mark_hole(file.as_raw_fd(), &hole);
        }
    }
}

fn main() {
    let args = Args::parse();

    if !args.file.exists() {
        eprintln!("File does not exist: {}", args.file.display());
        std::process::exit(1);
    }

    if args.dry_run {
        println!("Dry run");
    }

    let offset = args.offset.unwrap_or(0);
    let max_offset = args.length.map(|l| offset + l);

    Main {
        file: args.file,
        verbose: args.verbose,
        offset,
        max_offset,
        dry_run: args.dry_run,
    }
    .process();
}
