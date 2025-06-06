use io_uring::{IoUring, opcode, types};
use std::{io, os::fd::AsRawFd};

fn main() -> io::Result<()> {
    //const UDP: u8 = 0x01;
    const TCP: u8 = 0x02;
    const IN_PORT: [u8; 2] = 8080u16.to_be_bytes();
    const EX_PORT: [u8; 2] = IN_PORT;
    const TIMEOUT: [u8; 4] = 300u32.to_be_bytes(); //in secs, 0 to destroy mapping

    let mut request = vec![0x00, TCP, 0x00, 0x00];
    request.append(&mut IN_PORT.to_vec());
    request.append(&mut EX_PORT.to_vec());
    request.append(&mut TIMEOUT.to_vec());
    assert!(request.len() == 12);

    let sock = std::net::UdpSocket::bind("0.0.0.0:5350")?;
    sock.connect("192.168.1.1:5351")?;

    let mut uring = IoUring::new(4)?;
    let mut result_buf = vec![0u8; 16];
    let sock_fd = types::Fd(sock.as_raw_fd());

    let write_entry =
        opcode::Write::new(sock_fd, request.as_mut_ptr(), request.len() as _).build();
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
