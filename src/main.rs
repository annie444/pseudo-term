use nix::fcntl::OFlag;
use nix::pty::{PtyMaster, grantpt, posix_openpt, ptsname, unlockpt};
//use nix::sys::stat::Mode;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::os::fd::{FromRawFd, IntoRawFd};
//use std::path::Path;
use std::process::exit;

fn main() {
    let (master, slave) = match open_pty() {
        Ok(fd) => fd,
        Err(e) => {
            eprintln!("Failed to open PTY: {}", e);
            exit(1);
        }
    };

    let master_file = unsafe { File::from_raw_fd(master.into_raw_fd()) };

    println!("PTY opened successfully!");
    println!("Master FD: {:?}", master_file);
    println!("Slave FD: {:?}", slave);

    // let mut master_writer = BufWriter::new(
    //     master_file
    //         .try_clone()
    //         .expect("Failed to clone master file"),
    // );
    let master_reader = BufReader::new(master_file);

    for line in master_reader.lines() {
        if let Ok(line) = line {
            println!("Receiver: {}", line);
        } else {
            eprintln!("Failed to read from slave");
        }
    }
}

fn open_pty() -> nix::Result<(PtyMaster, String)> {
    let master_fd = posix_openpt(OFlag::O_RDWR)?;

    // Allow a slave to be generated for it
    grantpt(&master_fd)?;
    unlockpt(&master_fd)?;

    // Get the name of the slave
    let slave_name = unsafe { ptsname(&master_fd) }?;

    Ok((master_fd, slave_name))
}
