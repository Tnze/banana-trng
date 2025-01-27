use defmt::*;
use embassy_futures::{
    join::join,
    select::{select, Either},
};
use embassy_stm32::{
    gpio::{Level, Output, Speed},
    peripherals::{PA11, PA12, USB},
    usb::{Driver, Instance},
};
use embassy_sync::pubsub::DynSubscriber;
use embassy_time::Timer;
use embassy_usb::{
    class::cdc_acm::{CdcAcmClass, State},
    Builder,
};

use crate::{geiger, Irqs};

#[embassy_executor::task]
pub(crate) async fn run(
    pusb: USB,
    pa11: PA11,
    mut pa12: PA12,
    geiger_subscriber: DynSubscriber<'static, geiger::count::Message>,
) {
    {
        // Reset USB for development only
        let _dp = Output::new(&mut pa12, Level::Low, Speed::Low);
        Timer::after_millis(10).await;
    }

    let driver = Driver::new(pusb, Irqs, pa12, pa11);
    let mut config = embassy_usb::Config::new(0xc0de, 0xcafe);
    config.manufacturer = Some("Tnze");
    config.product = Some("Banana RNG");
    config.serial_number = Some("TNZ1");

    let mut config_descriptor = [0; 256];
    let mut bos_descriptor = [0; 256];
    let mut control_buf = [0; 32];

    let mut state = State::new();

    let mut builder = Builder::new(
        driver,
        config,
        &mut config_descriptor,
        &mut bos_descriptor,
        &mut [], // no msos descriptors
        &mut control_buf,
    );

    let mut class = CdcAcmClass::new(&mut builder, &mut state, 64);
    let mut usb = builder.build();
    let usb_fut = usb.run();
    let echo_fut = transfer(&mut class, geiger_subscriber);

    join(usb_fut, echo_fut).await;
}

async fn transfer<'d, T: Instance + 'd>(
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

async fn interacts<'d, T: Instance + 'd>(
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
