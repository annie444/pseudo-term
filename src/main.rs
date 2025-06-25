use anyhow::Result;
use clap::Parser;
use nix::cmsg_space;
use nix::sys::socket::{ControlMessageOwned, MsgFlags, RecvMsg, UnixAddr, recvmsg};
use nix::sys::termios::{InputFlags, OutputFlags, SetArg, cfmakeraw, tcgetattr, tcsetattr};
use std::fs::File;
use std::io::{BufRead, BufReader, IoSliceMut};
use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::process::exit;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::{JoinHandle, spawn};

/// A simple program for allocating pseudo-terminal (PTY) sockets
/// and handling communication with them.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// The path to where the PTY sockets should be created
    #[arg()]
    path: String,
}

/// Replication of the C function `receive_fd_from_socket_with_payload` in Rust.
/// This function receives a file descriptor over a Unix socket along with optional payload data.
/// Its C signature is:
/// ```c
/// int
/// receive_fd_from_socket_with_payload (int from, char *payload, size_t payload_len, libcrun_error_t *err);
/// ```
fn receive_fd_from_socket_with_payload(
    stream: UnixStream,
    payload: Option<&mut [u8]>,
) -> Result<RawFd> {
    let mut cmsg_buffer = cmsg_space!(RawFd);
    let mut data: [u8; 1] = [b' '];
    let mut iov = if let Some(payload) = payload {
        [IoSliceMut::new(payload)]
    } else {
        [IoSliceMut::new(&mut data)]
    };
    let received: RecvMsg<'_, '_, UnixAddr> = recvmsg(
        stream.as_raw_fd(),
        &mut iov,
        Some(&mut cmsg_buffer),
        MsgFlags::empty(),
    )?;
    let cmsg = match received
        .cmsgs()
        .map_err(|e| anyhow::anyhow!("No control message received, {}", e))?
        .next()
    {
        Some(cmsg) => cmsg,
        None => return Err(anyhow::anyhow!("No control message received")),
    };
    match cmsg {
        ControlMessageOwned::ScmRights(fds) => {
            if fds.is_empty() {
                return Err(anyhow::anyhow!("No file descriptor received"));
            }
            let fd = fds[0];
            // Here you can handle the received file descriptor, e.g., attach it to a PTY
            println!("Received file descriptor: {}", fd);
            Ok(fd)
        }
        _ => Err(anyhow::anyhow!(
            "Unexpected control message type: {:?}",
            cmsg
        )),
    }
}

fn set_term<Fd: AsFd>(fd: &Fd) -> Result<()> {
    let mut termios = tcgetattr(&fd)?;
    cfmakeraw(&mut termios);

    let iflags = termios.input_flags.bits() & OutputFlags::OPOST.bits();
    termios.input_flags = InputFlags::from_bits_retain(iflags);
    termios.output_flags &= OutputFlags::OPOST;

    tcsetattr(&fd, SetArg::TCSANOW, &termios)?;

    println!(
        "Terminal settings ({:?}) applied to fd ({})",
        &termios,
        fd.as_fd().as_raw_fd()
    );

    Ok(())
}

fn send_term(term: OwnedFd, term_tx: Sender<OwnedFd>) -> Result<()> {
    // Send the terminal file descriptor to the main thread
    if let Err(e) = term_tx.send(term) {
        eprintln!("Failed to send terminal fd: {}", e);
        return Err(anyhow::anyhow!("Failed to send terminal fd"));
    } else {
        println!("Terminal fd sent successfully");
    }
    Ok(())
}

fn handle_client(stream: UnixStream, term_tx: Sender<OwnedFd>) -> Result<()> {
    let term = receive_fd_from_socket_with_payload(stream, None)?;
    let term = unsafe { OwnedFd::from_raw_fd(term) };
    set_term(&term)?;
    println!("Terminal fd: {}", term.as_fd().as_raw_fd());
    send_term(term, term_tx)?;
    Ok(())
}

fn main() {
    let args = Cli::parse();
    let (term_tx, term_rx): (Sender<OwnedFd>, Receiver<OwnedFd>) = channel();
    let mut threads: Vec<JoinHandle<Result<()>>> = Vec::new();
    let socket = UnixListener::bind(&args.path).unwrap_or_else(|e| {
        eprintln!("Failed to bind to {}: {}", args.path, e);
        exit(1);
    });
    println!("Listening for connections on {}", args.path);
    threads.push(spawn(move || handle_terminal(term_rx)));
    for stream in socket.incoming() {
        match stream {
            Ok(stream) => {
                println!("New connection: {:?}", stream);
                let tx = term_tx.clone();
                threads.push(spawn(move || handle_client(stream, tx)));
            }
            Err(e) => {
                eprintln!("Connection failed: {}", e);
            }
        }
    }
    for thread in threads {
        if let Err(e) = thread.join().unwrap() {
            eprintln!("Error handling client: {}", e);
        }
    }
}

fn get_terminal(term_rx: Receiver<OwnedFd>) -> OwnedFd {
    let mut term_fd: Option<OwnedFd> = None;
    while term_fd.is_none() {
        if let Ok(term) = term_rx.recv() {
            term_fd = Some(term);
        }
    }
    term_fd.unwrap()
}

fn show_term(term: OwnedFd) -> Result<()> {
    let term_file = unsafe { File::from_raw_fd(term.as_raw_fd()) };
    let mut term_reader = BufReader::new(term_file);
    let mut term_output = String::new();
    while let Ok(line) = term_reader.read_line(&mut term_output) {
        if line == 0 {
            continue;
        }
        print!("{}", term_output);
        term_output.clear();
    }
    Ok(())
}

fn handle_terminal(term_rx: Receiver<OwnedFd>) -> Result<()> {
    let term = get_terminal(term_rx);
    show_term(term)?;
    Ok(())
}
