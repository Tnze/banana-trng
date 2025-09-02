use defmt::*;
use embassy_futures::select::{select, Either};
use embassy_stm32::usb::{Driver, Instance};
use embassy_sync::pubsub::DynSubscriber;
use embassy_usb::class::cdc_acm::CdcAcmClass;

use crate::geiger;

pub(super) async fn transfer<'d, T: Instance + 'd>(
    class: &mut CdcAcmClass<'d, Driver<'d, T>>,
    mut geiger_subscriber: DynSubscriber<'static, geiger::count::Message>,
) {
    loop {
        class.wait_connection().await;
        info!("Connected");
        let _ = interacts(class, &mut geiger_subscriber).await;
        info!("Disconnected");
    }
}

pub(super) async fn interacts<'d, T: Instance + 'd>(
    class: &mut CdcAcmClass<'d, Driver<'d, T>>,
    geiger_subscriber: &mut DynSubscriber<'static, geiger::count::Message>,
) {
    use core::fmt::Write;
    let mut line_buffer = [0u8; 128];
    let mut line = heapless::Vec::<u8, 64>::new();
    loop {
        match select(
            class.read_packet(&mut line_buffer),
            geiger_subscriber.next_message_pure(),
        )
        .await
        {
            Either::First(_result) => {
                // if let Ok(n) = result {
                //     info!("Read {} bytes {:a}", n, line_buffer[..n]);
                // }
            }
            Either::Second(geiger::count::Message { dur, cpm, val }) => {
                if core::write!(&mut line, "Dur:{dur} ms CPM:{cpm} RD:{val:.5} uSv/h\n").is_ok() {
                    if let Ok(()) = class.write_packet(&line).await {
                        info!("Write {} bytes", line.len());
                    }
                    line.clear();
                }
            }
        }
    }
}
