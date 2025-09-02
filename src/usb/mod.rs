mod cli;
mod uart;

use embassy_futures::join::join3;
use embassy_stm32::{
    gpio::{Level, Output, Speed},
    mode::Async,
    peripherals::{PA11, PA12, USB},
    usart::Uart,
    usb::Driver,
    Peri,
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
    pusb: Peri<'static, USB>,
    pa11: Peri<'static, PA11>,
    mut pa12: Peri<'static, PA12>,
    uart: Uart<'static, Async>,
    geiger_subscriber: DynSubscriber<'static, geiger::count::Message>,
) {
    {
        // Reset USB for development only
        let _dp = Output::new(pa12.reborrow(), Level::Low, Speed::Low);
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

    let mut cli_state = State::new();
    let mut uart_state = State::new();

    let mut builder = Builder::new(
        driver,
        config,
        &mut config_descriptor,
        &mut bos_descriptor,
        &mut [], // no msos descriptors
        &mut control_buf,
    );

    let mut cli_class = CdcAcmClass::new(&mut builder, &mut cli_state, 64);
    let uart_class = CdcAcmClass::new(&mut builder, &mut uart_state, 64);
    let mut usb = builder.build();
    let usb_fut = usb.run();
    let cli_fut = cli::transfer(&mut cli_class, geiger_subscriber);
    let uart_fut = uart::uart_transfer(uart_class, uart);

    join3(usb_fut, cli_fut, uart_fut).await;
}
