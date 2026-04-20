use std::fs::OpenOptions;
use std::io::{self, Read, Write};
use std::mem;
use std::os::fd::AsRawFd;

const LIRC_MODE_MODE2: u32 = 0x00000004;
const LIRC_MODE_PULSE: u32 = 0x00000002;
const LIRC_MODE2_FREQUENCY: u32 = 0x02000000;
const LIRC_MODE2_MASK: u32 = 0xFF000000;
const LIRC_MODE2_OVERFLOW: u32 = 0x04000000;
const LIRC_MODE2_PULSE: u32 = 0x01000000;
const LIRC_MODE2_SPACE: u32 = 0x00000000;
const LIRC_MODE2_TIMEOUT: u32 = 0x03000000;
const LIRC_SET_REC_MODE: libc::c_ulong = 0x40046912;
const LIRC_SET_REC_TIMEOUT_REPORTS: libc::c_ulong = 0x40046919;
const LIRC_SET_REC_TIMEOUT: libc::c_ulong = 0x40046918;
const LIRC_SET_SEND_CARRIER: libc::c_ulong = 0x40046913;
const LIRC_SET_SEND_DUTY_CYCLE: libc::c_ulong = 0x40046915;
const LIRC_SET_SEND_MODE: libc::c_ulong = 0x40046911;
const LIRC_VALUE_MASK: u32 = 0x00FFFFFF;

fn main() {
    println!("Hello, world!");
}

fn ir_send(path: &str, signal: &[u32], carrier_hz: u32, duty_cycle: u32) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).open(path)?;
    let fd = file.as_raw_fd();

    let mut lirc_mode = LIRC_MODE_PULSE;
    let mut carrier_hz = carrier_hz;
    let mut duty_cycle = duty_cycle;
    ioctl(fd, LIRC_SET_SEND_MODE, &mut lirc_mode)?;
    ioctl(fd, LIRC_SET_SEND_CARRIER, &mut carrier_hz)?;
    ioctl(fd, LIRC_SET_SEND_DUTY_CYCLE, &mut duty_cycle)?;

    let mut buf = Vec::with_capacity(mem::size_of_val(signal));
    for &t in signal {
        buf.extend_from_slice(&t.to_ne_bytes());
    }
    file.write_all(&buf)?;

    Ok(())
}

fn ir_recv(path: &str, timeout_us: u32) -> io::Result<Vec<u32>> {
    let mut file = OpenOptions::new().read(true).open(path)?;
    let fd = file.as_raw_fd();

    let mut lirc_mode = LIRC_MODE_MODE2;
    let mut lirc_timeout = timeout_us;
    let mut lirc_timeout_reports = 1u32;
    ioctl(fd, LIRC_SET_REC_MODE, &mut lirc_mode)?;
    ioctl(fd, LIRC_SET_REC_TIMEOUT, &mut lirc_timeout)?;
    ioctl(fd, LIRC_SET_REC_TIMEOUT_REPORTS, &mut lirc_timeout_reports)?;

    let mut started = false;
    let mut timings_us = Vec::new();
    let mut buf = [0u8; 4];
    loop {
        file.read_exact(&mut buf)?;
        let packet = u32::from_ne_bytes(buf);
        let kind = packet & LIRC_MODE2_MASK;
        let value = packet & LIRC_VALUE_MASK;
        match kind {
            LIRC_MODE2_PULSE => {
                started = true;
                timings_us.push(value);
            }
            LIRC_MODE2_SPACE => {
                if started {
                    timings_us.push(value);
                }
            }
            LIRC_MODE2_TIMEOUT => {
                if started {
                    break;
                }
            }
            LIRC_MODE2_FREQUENCY => {}
            LIRC_MODE2_OVERFLOW => {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "overflow"));
            }
            _ => {}
        }
    }

    if timings_us.len() % 2 == 0 {
        timings_us.pop();
    }

    if timings_us.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "empty IR frame"));
    }

    Ok(timings_us)
}

fn ioctl(fd: i32, request: libc::c_ulong, value: &mut u32) -> io::Result<()> {
    let rc = unsafe { libc::ioctl(fd, request, value as *mut u32) };
    if rc == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
