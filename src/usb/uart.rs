use defmt::*;
use embassy_futures::{join::join, select::select};
use embassy_stm32::{
    mode::Async,
    usart::{Config, DataBits, Parity, StopBits, Uart},
    usb::{Driver, Instance},
};
use embassy_usb::class::cdc_acm::{
    CdcAcmClass, LineCoding, ParityType as ParityTypeACM, StopBits as StopBitsACM,
};

pub(super) async fn uart_transfer<'d, T: Instance + 'd>(
    class: CdcAcmClass<'d, Driver<'d, T>>,
    mut uart: Uart<'static, Async>,
) {
    info!("Uart transfer is running");
    let (mut sender, mut receiver, control) = class.split_with_control();
    let (tx, rx) = uart.split_ref();
    loop {
        sender.wait_connection().await;
        info!("CDC-ACM connection detected");

        let line_coding = sender.line_coding();
        info!("CDC-ACM line coding config: {:?}", line_coding);
        let config = line_coding_to_uart_config(&line_coding);
        tx.set_config(&config).unwrap();
        rx.set_config(&config).unwrap();
        select(
            async {
                control.control_changed().await;
                info!("Control changed");
            },
            join(
                async {
                    let mut buffer = [0u8; 64];
                    loop {
                        match receiver.read_packet(&mut buffer).await {
                            Ok(n) => tx.write(&buffer[..n]).await.unwrap(),
                            Err(err) => error!("Read from CDC-ACM error: {:?}", err),
                        }
                    }
                },
                async {
                    let mut buffer = [0u8; 64];
                    loop {
                        match rx.read_until_idle(&mut buffer).await {
                            Ok(n) => sender.write_packet(&buffer[..n]).await.unwrap(),
                            Err(err) => error!("Read from USART error: {:?}", err),
                        }
                    }
                },
            ),
        )
        .await;
    }
}

fn line_coding_to_uart_config(line_coding: &LineCoding) -> Config {
    let mut config = Config::default();
    config.baudrate = line_coding.data_rate();
    config.data_bits = match line_coding.data_bits() {
        7 => DataBits::DataBits7,
        8 => DataBits::DataBits8,
        9 => DataBits::DataBits9,
        _ => DataBits::DataBits8,
    };
    config.stop_bits = match line_coding.stop_bits() {
        StopBitsACM::One => StopBits::STOP1,
        StopBitsACM::OnePointFive => StopBits::STOP1P5,
        StopBitsACM::Two => StopBits::STOP2,
    };
    config.parity = match line_coding.parity_type() {
        ParityTypeACM::None | ParityTypeACM::Mark | ParityTypeACM::Space => Parity::ParityNone,
        ParityTypeACM::Odd => Parity::ParityOdd,
        ParityTypeACM::Even => Parity::ParityEven,
    };
    config
}
