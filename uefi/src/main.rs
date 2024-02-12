#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec::Vec;
use alloc::{format, vec};
use core::fmt::Write;

use log::{info, warn};
use trebuchet_lib::CHUNK_SIZE;
use uefi::prelude::*;
use uefi::proto::console::serial::Serial;
use uefi::table::boot::{LoadImageSource, OpenProtocolAttributes, OpenProtocolParams};

#[entry]
fn main(_image_handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&mut system_table).unwrap();
    let boot_services = system_table.boot_services();
    bootloader_main(boot_services);
}

fn bootloader_main(boot_services: &BootServices) -> ! {
    info!("Trebuchet UEFI: the StoneOS UEFI chain-loader");
    {
        let image_buffer = receive_image(boot_services);
        let image_source = LoadImageSource::FromBuffer {
            buffer: &image_buffer,
            file_path: None,
        };

        info!("Loading received image.");
        let image_handle = boot_services
            .load_image(boot_services.image_handle(), image_source)
            .expect("Failed to load received image.");

        info!("Starting loaded image.");
        boot_services
            .start_image(image_handle)
            .expect("Failed to start loaded image.");
    }
    unreachable!("Trebuchet UEFI: returned to chain loader after image start.")
}

fn receive_image(boot_services: &BootServices) -> Vec<u8> {
    info!("Loading image over serial.");

    info!("Opening serial communication.");
    let serial_handle = boot_services
        .get_handle_for_protocol::<Serial>()
        .expect("Serial protocol unavailable.");

    // Avoid opening the protocol exclusively to keep text input and output to serial (e.g. logging) possible.
    let mut serial = unsafe {
        let params = OpenProtocolParams {
            handle: serial_handle,
            agent: boot_services.image_handle(),
            controller: None,
        };

        boot_services
            .open_protocol::<Serial>(params, OpenProtocolAttributes::GetProtocol)
            .expect("Failed to open serial protocol.")
    };

    serial.reset().expect("Failed to reset the serial device.");

    info!("Requesting image.");
    serial
        .write_str(&format!("RDY({})\n", CHUNK_SIZE))
        .expect("Failed to send image request.");

    let mut size_buffer = [0u8; core::mem::size_of::<u64>()];
    receive_bytes(&mut serial, &mut size_buffer);

    let size_checksum = trebuchet_lib::checksum(&size_buffer);
    serial
        .write_str(&format!("OK({})\n", size_checksum))
        .unwrap_or_else(|_| warn!("Failed to send image size confirmation."));

    let size = usize::from_be_bytes(size_buffer);
    info!("Expected image size: {} bytes.", size);

    info!("Waiting for image.");
    let mut image_buffer = vec![0u8; size];
    receive_bytes(&mut serial, &mut image_buffer);

    let image_checksum = trebuchet_lib::checksum(&image_buffer);
    serial
        .write_str(&format!("OK({})\n", image_checksum))
        .unwrap_or_else(|_| warn!("Failed to send image receipt confirmation."));

    info!("Received image.");
    image_buffer
}

fn receive_bytes(serial: &mut Serial, data: &mut [u8]) {
    for (i, chunk) in data.chunks_mut(CHUNK_SIZE).enumerate() {
        while serial.read(chunk).is_err() {}
        serial
            .write_str(&format!("ACK({})\n", i))
            .unwrap_or_else(|_| warn!("Failed to send byte receipt confirmation."));
    }
}
