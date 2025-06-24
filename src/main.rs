use clap::Parser;
use nix::cmsg_space;
use nix::sys::socket::{ControlMessageOwned, MsgFlags, RecvMsg, UnixAddr, recvmsg};
use std::io::IoSliceMut;
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::process::exit;
use std::thread;

/// A simple program for allocating pseudo-terminal (PTY) sockets
/// and handling communication with them.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// The path to where the PTY sockets should be created
    #[arg()]
    path: String,
}

fn handle_client(stream: UnixStream) {
    let mut cmsg_buffer = cmsg_space!(RawFd);
    let mut data_buffer = [0u8; 1024]; // Buffer to hold incoming data
    let mut iov = [IoSliceMut::new(&mut data_buffer)];
    let received: RecvMsg<'_, '_, UnixAddr> = recvmsg(
        stream.as_raw_fd(),
        &mut iov,
        Some(&mut cmsg_buffer),
        MsgFlags::empty(),
    )
    .unwrap();
    if let Ok(cmsgs) = received.cmsgs() {
        for cmsg in cmsgs {
            match cmsg {
                ControlMessageOwned::ScmTimestamp(timestamp) => {
                    // Handle timestamp
                    println!("Received timestamp: {:?}", timestamp);
                }
                ControlMessageOwned::ScmRights(fds) => {
                    // Handle file descriptors
                    for fd in fds {
                        println!("Received file descriptor: {}", fd);
                        // Here you can handle the received file descriptor, e.g., attach it to a PTY
                    }
                }
                // ... Handle other control message types
                _ => {
                    // Handle unrecognized cmsg types
                    eprintln!("Received unrecognized control message: {:?}", cmsg);
                }
            }
        }
    }
}

fn main() {
    let args = Cli::parse();
    let socket = UnixListener::bind(&args.path).unwrap_or_else(|e| {
        eprintln!("Failed to bind to {}: {}", args.path, e);
        exit(1);
    });
    println!("Listening for connections on {}", args.path);
    for stream in socket.incoming() {
        match stream {
            Ok(stream) => {
                println!("New connection: {:?}", stream);
                thread::spawn(|| handle_client(stream));

                // Here you can handle the communication with the PTY
                // For example, you could spawn a child process and connect it to the PTY
            }
            Err(e) => {
                eprintln!("Connection failed: {}", e);
            }
        }
    }
}
