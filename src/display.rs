use core::fmt::Write;

use display_interface::AsyncWriteOnlyDataCommand;
use embassy_stm32::{
    gpio,
    peripherals::{DMA1_CH3, PA0, PA1, PA4, PA5, PA7, SPI1},
    spi::Spi,
};
use embassy_sync::pubsub::DynSubscriber;
use ssd1306::{
    mode::{TerminalModeAsync, TerminalModeError},
    prelude::*,
    Ssd1306Async,
};

use crate::geiger;

#[embassy_executor::task]
pub(crate) async fn run(
    spi1: SPI1,
    sck: PA5,
    mosi: PA7,
    dma1_ch3: DMA1_CH3,
    rst: PA0,
    dc: PA1,
    cs: PA4,
    mut geiger_subscriber: DynSubscriber<'static, geiger::count::Message>,
) {
    let mut rst = gpio::Output::new(rst, gpio::Level::Low, gpio::Speed::Low);
    let dc = gpio::Output::new(dc, gpio::Level::Low, gpio::Speed::Low);
    let spi = embedded_hal_bus::spi::ExclusiveDevice::new_no_delay(
        Spi::new_txonly(spi1, sck, mosi, dma1_ch3, Default::default()),
        gpio::Output::new(cs, gpio::Level::Low, gpio::Speed::Low),
    )
    .unwrap();
    let interface = SPIInterface::new(spi, dc);
    let mut display = Ssd1306Async::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
        .into_terminal_mode();

    display
        .reset(&mut rst, &mut embassy_time::Delay)
        .await
        .unwrap();
    if let Err(err) = try_display(&mut display, &mut geiger_subscriber).await {
        defmt::error!("Failed to drive display: {:?}", defmt::Debug2Format(&err));
    }
}

async fn try_display(
    display: &mut Ssd1306Async<
        impl AsyncWriteOnlyDataCommand,
        DisplaySize128x64,
        TerminalModeAsync,
    >,
    geiger_subscriber: &mut DynSubscriber<'static, geiger::count::Message>,
) -> Result<(), TerminalModeError> {
    display.init().await?;
    display.clear().await?;
    display.write_str("hello, world").await?;
    let mut line = heapless::String::<64>::new();
    loop {
        let geiger::count::Message { dur, cpm, val } = geiger_subscriber.next_message_pure().await;
        if write!(&mut line, "Dur:{dur} ms\nCPM:{cpm}\nRD:{val:.5} uSv/h\n").is_ok() {
            display.clear().await?;
            display.write_str(&line).await?;
            line.clear();
        }
    }
}
