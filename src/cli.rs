use defmt::*;
use embassy_stm32::usb::{Driver, Instance};
use embassy_usb::class::cdc_acm::CdcAcmClass;

pub(crate) async fn run<'d, T: Instance + 'd>(class: &mut CdcAcmClass<'d, Driver<'d, T>>) {
    loop {
        class.wait_connection().await;
        info!("Connected");
        let _ = interacts(class).await;
        info!("Disconnected");
    }
}

async fn interacts<'d, T: Instance + 'd>(class: &mut CdcAcmClass<'d, Driver<'d, T>>) {
    let mut line_buffer = [0u8; 128];
    while let Ok(n) = class.read_packet(&mut line_buffer).await {
        info!("Read {} bytes", n);
    }
}
