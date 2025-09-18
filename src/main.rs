#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_stm32::{
    adc::Adc,
    bind_interrupts,
    peripherals::{ADC1, USART1, USART3, USB},
    time::Hertz,
    usart::Uart,
    Config,
};
use embassy_sync::{
    blocking_mutex::raw::{NoopRawMutex, ThreadModeRawMutex},
    mutex::Mutex,
    pubsub::PubSubChannel,
};
use sequential_storage::cache::NoCache;
use static_cell::StaticCell;

use {defmt_rtt as _, panic_probe as _};

mod display;
mod geiger;
mod storage;
mod usb;

bind_interrupts!(
    struct Irqs {
        ADC1_2 => embassy_stm32::adc::InterruptHandler<ADC1>;
        USB_LP_CAN1_RX0 => embassy_stm32::usb::InterruptHandler<USB>;
        USART1 => embassy_stm32::usart::InterruptHandler<USART1>;
        USART3 => embassy_stm32::usart::InterruptHandler<USART3>;
    }
);

static GEIGER_PUBLISHER: StaticCell<PubSubChannel<NoopRawMutex, geiger::count::Message, 5, 2, 1>> =
    StaticCell::new();
static STORAGE: StaticCell<Mutex<ThreadModeRawMutex, storage::Storage<NoCache, 32>>> =
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

    let storage = storage::Storage::new(p.FLASH, storage::NoCache::new());
    let storage = STORAGE.init(Mutex::new(storage));

    let geiger_channel =
        GEIGER_PUBLISHER
            .init(PubSubChannel::<NoopRawMutex, geiger::count::Message, 5, 2, 1>::new());

    #[cfg(not(feature = "uart3_cdc"))]
    let debug_uart = Uart::new(
        p.USART1,
        p.PA10,
        p.PA9,
        Irqs,
        p.DMA1_CH4,
        p.DMA1_CH5,
        Default::default(),
    )
    .unwrap();

    #[cfg(feature = "uart3_cdc")]
    let debug_uart = Uart::new(
        p.USART3,
        p.PB11,
        p.PB10,
        Irqs,
        p.DMA1_CH2,
        p.DMA1_CH3,
        Default::default(),
    )
    .unwrap();

    spawner.spawn(
        usb::run(
            p.USB,
            p.PA11,
            p.PA12,
            debug_uart,
            geiger_channel.dyn_subscriber().unwrap(),
        )
        .expect("Failed to spawn debug_uart task"),
    );
    spawner.spawn(
        geiger::run(
            Adc::new(p.ADC1),
            p.PB0,
            p.PB9,
            p.TIM4,
            p.PB8,
            p.EXTI8,
            geiger_channel.dyn_publisher().unwrap(),
            storage,
        )
        .expect("Failed to spawn geiger driver task"),
    );
    #[cfg(not(feature = "uart3_cdc"))]
    spawner.spawn(
        display::run(
            p.SPI1,
            p.PA5,
            p.PA7,
            p.DMA1_CH3,
            p.PA0,
            p.PA1,
            p.PA4,
            geiger_channel.dyn_subscriber().unwrap(),
        )
        .expect("Failed to spawn display driver task"),
    );
}
