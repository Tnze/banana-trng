use display_interface::AsyncWriteOnlyDataCommand;
use embassy_stm32::{
    gpio,
    peripherals::{DMA1_CH3, PA0, PA1, PA4, PA5, PA7, SPI1},
    spi::Spi,
};
use ssd1306::{
    mode::{TerminalModeAsync, TerminalModeError},
    prelude::*,
    Ssd1306Async,
};

#[embassy_executor::task]
pub async fn run(spi1: SPI1, sck: PA5, mosi: PA7, dma1_ch3: DMA1_CH3, rst: PA0, dc: PA1, cs: PA4) {
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
    loop {
        display
            .reset(&mut rst, &mut embassy_time::Delay)
            .await
            .unwrap();
        let _ = try_display(&mut display).await;
    }
}

async fn try_display(
    display: &mut Ssd1306Async<
        impl AsyncWriteOnlyDataCommand,
        DisplaySize128x64,
        TerminalModeAsync,
    >,
) -> Result<(), TerminalModeError> {
    display.init().await?;
    display.clear().await?;
    loop {
        display.write_str("Tnze\n").await?;
    }
}
