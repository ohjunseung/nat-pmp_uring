use clap::{Parser, ValueEnum, arg};
use io_uring::{IoUring, opcode, types};
use std::{io, os::fd::AsRawFd};

#[derive(Copy, Clone, Debug, ValueEnum)]
#[repr(u8)]
enum Protocol {
    TCP = 0x01,
    UDP = 0x02,
}

#[derive(Parser, Debug)]
struct Args {
    #[arg(short, value_enum, default_value_t = Protocol::TCP)]
    protocol: Protocol,
    internal_port: u16,
    external_port: Option<u16>,

    #[arg(short, default_value_t = 0)]
    timeout: u32,
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    let protocol: u8 = args.protocol as u8;
    let in_port: [u8; 2] = args.internal_port.to_be_bytes();
    let ex_port: [u8; 2] = match args.external_port {
        Some(port) => port.to_be_bytes(),
        None => in_port,
    };
    let timeout: [u8; 4] = args.timeout.to_be_bytes(); //in secs, 0 to destroy mapping

    let mut request = vec![0x00, protocol, 0x00, 0x00];
    request.append(&mut in_port.to_vec());
    request.append(&mut ex_port.to_vec());
    request.append(&mut timeout.to_vec());
    assert!(request.len() == 12);

    let sock = std::net::UdpSocket::bind("0.0.0.0:5350")?;
    sock.connect("192.168.1.1:5351")?;

    let mut uring = IoUring::new(4)?;
    let mut result_buf = vec![0u8; 16];
    let sock_fd = types::Fd(sock.as_raw_fd());

    let write_entry = opcode::Write::new(sock_fd, request.as_mut_ptr(), request.len() as _).build();
    let read_entry =
        opcode::Read::new(sock_fd, result_buf.as_mut_ptr(), result_buf.len() as _).build();

    unsafe {
        uring.submission().push(&write_entry).expect("sqe full");
        uring.submission().push(&read_entry).expect("sqe full");
    }

    uring.submit_and_wait(2)?;

    let mut cqe = uring.completion();
    let result = cqe.next().expect("no value");
    assert!(result.result() >= 0, "failed completion");

    cqe.next().expect("no value");
    assert!(result.result() >= 0, "failed completion");

    let version = result_buf[0];
    let op = result_buf[1];
    let (result_code, rest) = result_buf[2..].split_at(std::mem::size_of::<u16>());
    let (epoch, rest) = rest.split_at(std::mem::size_of::<u32>());
    let (internal, rest) = rest.split_at(std::mem::size_of::<u16>());
    let (external, lifetime) = rest.split_at(std::mem::size_of::<u16>());

    println!("Version: {:x}", version);
    println!("OP: {:x}", op);
    println!(
        "Result Code: {:x}",
        u16::from_be_bytes(result_code.try_into().unwrap())
    );
    println!("Epoch: {}", u32::from_be_bytes(epoch.try_into().unwrap()));
    println!(
        "Internal Port: {}",
        u16::from_be_bytes(internal.try_into().unwrap())
    );
    println!(
        "External Port: {}",
        u16::from_be_bytes(external.try_into().unwrap())
    );
    println!(
        "Lifetime: {} secs",
        u32::from_be_bytes(lifetime.try_into().unwrap())
    );
    Ok(())
}
