#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::{
    adc::Adc,
    bind_interrupts,
    peripherals::{self, ADC1},
    time::Hertz,
    Config,
};
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, pubsub::PubSubChannel};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

mod cli;
mod display;
mod geiger;

bind_interrupts!(
    struct Irqs {
        ADC1_2 => embassy_stm32::adc::InterruptHandler<ADC1>;
        USB_LP_CAN1_RX0 => embassy_stm32::usb::InterruptHandler<peripherals::USB>;
    }
);

static GEIGER_PUBLISHER: StaticCell<PubSubChannel<NoopRawMutex, geiger::Message, 5, 1, 1>> =
    StaticCell::new();

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let mut config = Config::default();
    {
        use embassy_stm32::rcc::*;
        config.rcc.hse = Some(Hse {
            freq: Hertz::mhz(8),
            mode: HseMode::Oscillator,
        });
        config.rcc.pll = Some(Pll {
            src: PllSource::HSE,
            prediv: PllPreDiv::DIV1,
            mul: PllMul::MUL9,
        });
        config.rcc.sys = Sysclk::PLL1_P;
        config.rcc.ahb_pre = AHBPrescaler::DIV1;
        config.rcc.apb1_pre = APBPrescaler::DIV2;
        config.rcc.apb2_pre = APBPrescaler::DIV1;
    }
    let p = embassy_stm32::init(config);

    info!("Hello World!");

    let geiger_channel =
        GEIGER_PUBLISHER.init(PubSubChannel::<NoopRawMutex, geiger::Message, 5, 1, 1>::new());

    spawner.must_spawn(cli::run(p.USB, p.PA11, p.PA12));
    spawner.must_spawn(geiger::run(
        Adc::new(p.ADC1),
        p.PB0,
        p.PB9,
        p.TIM4,
        p.PB8,
        p.EXTI8,
        geiger_channel.dyn_publisher().unwrap(),
    ));
    spawner.must_spawn(display::run(
        p.SPI1,
        p.PA5,
        p.PA7,
        p.DMA1_CH3,
        p.PA0,
        p.PA1,
        p.PA4,
        geiger_channel.dyn_subscriber().unwrap(),
    ));
}
