use std::env::VarError;
use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use clap::Parser;
use indicatif::{MultiProgress, ProgressBar, ProgressState, ProgressStyle};
use indicatif_log_bridge::LogWrapper;
use log::{debug, info, trace};
use serialport::SerialPort;

use trebuchet_lib::CHUNK_SIZE;

static SERIAL_TIMEOUT: Duration = Duration::from_micros(10);

#[derive(Parser)]
struct Cli {
    serial_port: String,
    serial_baud: u32,
    image_path: PathBuf,
}

fn main() {
    println!("{}", trebuchet_lib::SPLASH);

    if let Err(VarError::NotPresent) = std::env::var("RUST_LOG") {
        std::env::set_var("RUST_LOG", "debug");
    }
    let logger = env_logger::Builder::from_env(env_logger::Env::default()).build();

    let multi = MultiProgress::new();
    LogWrapper::new(multi.clone(), logger)
        .try_init()
        .expect("Failed to initialize logger.");

    let args = Cli::parse();
    let image = open_image_file(&args.image_path);
    let timeout = Some(Duration::from_millis(30));
    let mut port = open_serial_port(&args.serial_port, args.serial_baud);

    info!("Waiting for RDY signal.");
    wait_for_bytes(
        port.as_mut(),
        format!("RDY({})\n", CHUNK_SIZE).as_bytes(),
        None,
    )
    .expect("Failed to receive RDY.");

    let size = &(image.len() as u64).to_be_bytes();
    debug!("Transmitting image size.");
    port.write_all(size)
        .expect("Failed to transmit image size.");

    for i in 0..size.chunks(CHUNK_SIZE).len() {
        wait_for_bytes(port.as_mut(), format!("ACK({})\n", i).as_bytes(), timeout)
            .expect("Timed out waiting for image size ACK.");
        trace!("ACK({})\n", i);
    }

    let size_checksum = trebuchet_lib::checksum(size);
    debug!("Waiting for image size OK({}).", size_checksum);
    wait_for_bytes(
        port.as_mut(),
        format!("OK({})\n", size_checksum).as_bytes(),
        timeout,
    )
    .expect("Timed out waiting for image size OK.");
    trace!("OK({})\n", size_checksum);

    info!("Transmitting image.");
    let progress = multi.add(ProgressBar::new(image.len() as u64));
    progress.set_style(ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{bar:60.cyan/blue}] {bytes}/{total_bytes} ({eta})")
        .unwrap()
        .with_key(
            "eta",
            |state: &ProgressState, w: &mut dyn Write|
                { write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap() })
        .progress_chars("#>-"));

    for (i, chunk) in image.chunks(CHUNK_SIZE).enumerate() {
        loop {
            let _ = port.write_all(chunk);
            if wait_for_bytes(port.as_mut(), format!("ACK({})\n", i).as_bytes(), timeout).is_ok() {
                trace!("ACK({})\n", i);
                break;
            }
        }
        progress.inc(CHUNK_SIZE as u64);
    }
    progress.finish();
    multi.remove(&progress);

    let image_checksum = trebuchet_lib::checksum(&image);
    debug!("Waiting for image OK({}).", image_checksum);
    wait_for_bytes(
        port.as_mut(),
        format!("OK({})\n", image_checksum).as_bytes(),
        Some(Duration::from_millis(20)),
    )
    .expect("Timed out waiting for image OK.");
    trace!("OK({})\n", image_checksum);

    info!("Transmission complete.");

    loop {
        let mut c = 0u8;
        if port.read(std::slice::from_mut(&mut c)).is_ok() {
            print!("{}", c as char);
        }
    }
}

fn open_image_file(path: &Path) -> Vec<u8> {
    std::fs::read(path).expect("Failed to read image file")
}

fn open_serial_port(port: &str, baud: u32) -> Box<dyn SerialPort> {
    serialport::new(port, baud)
        .timeout(SERIAL_TIMEOUT)
        .open()
        .expect("Failed to open serial port")
}

fn wait_for_bytes(
    port: &mut dyn SerialPort,
    bytes: &[u8],
    timeout: Option<Duration>,
) -> Result<(), usize> {
    let mut remaining = timeout;
    let mut i = 0;
    while i < bytes.len() && (timeout.is_none() || remaining.is_some_and(|it| it > Duration::ZERO))
    {
        let mut c = 0u8;
        if port.read(core::slice::from_mut(&mut c)).is_ok() {
            if bytes[i] == c {
                i += 1;
            } else {
                i = 0;
            }
        } else if timeout.is_some() {
            remaining = remaining.map(|it| it.saturating_sub(port.timeout()));
        }
    }

    if i == bytes.len() {
        Ok(())
    } else {
        Err(i)
    }
}
