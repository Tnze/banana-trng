//! Blinks an LED
//!
//! This assumes that a LED is connected to pc13 as is the case on the blue pill board.
//!
//! Note: Without additional hardware, PC13 should not be used to drive an LED, see page 5.1.2 of
//! the reference manual for an explanation. This is not an issue on the blue pill.

// #![deny(unsafe_code)]
#![no_main]
#![no_std]
#![feature(sync_unsafe_cell)]

use core::cell::{OnceCell, RefCell};

use cortex_m_rt::entry;
use cortex_m_semihosting::hprintln;
use critical_section::Mutex;
use panic_halt as _;
use stm32f1xx_hal::{
    adc,
    gpio::{Edge, ExtiPin},
    pac::{self, interrupt},
    prelude::*,
    timer::Tim4NoRemap,
};

mod geiger;

use geiger::Geiger;

static GEIGER: Mutex<OnceCell<RefCell<Geiger<3, 'B', 8>>>> = Mutex::new(OnceCell::new());

#[interrupt]
fn EXTI9_5() {
    critical_section::with(|cs| {
        if let Some(geiger) = GEIGER.borrow(cs).get() {
            geiger.borrow_mut().interrupt();
        }
    });
}

#[entry]
fn main() -> ! {
    hprintln!("Hello, world!");

    // Get access to the core peripherals from the cortex-m crate
    let cp = cortex_m::Peripherals::take().unwrap();
    // Get access to the device specific peripherals from the peripheral access crate
    let mut dp = pac::Peripherals::take().unwrap();

    let mut flash = dp.FLASH.constrain();
    let rcc = dp.RCC.constrain();
    let clocks = rcc.cfgr.freeze(&mut flash.acr);
    let mut afio = dp.AFIO.constrain();
    let mut delay = cp.SYST.delay(&clocks);
    let mut gpiob = dp.GPIOB.split();

    // Init Geiger Tube driver
    {
        let mut geiger_out = gpiob.pb8.into_floating_input(&mut gpiob.crh);
        geiger_out.make_interrupt_source(&mut afio);
        geiger_out.trigger_on_edge(&mut dp.EXTI, Edge::Falling);
        geiger_out.enable_interrupt(&mut dp.EXTI);
        let adc1 = adc::Adc::adc1(dp.ADC1, &clocks);
        let geiger_boost_feedback = gpiob.pb0.into_analog(&mut gpiob.crl);
        // let mut geiger_boost_out = gpiob.pb9.into_push_pull_output(&mut gpiob.crh);
        // geiger_boost_out.set_high();
        let geiger_boost_out = gpiob.pb9.into_alternate_push_pull(&mut gpiob.crh);
        let geiger_boost_pwm = dp.TIM4.pwm_hz::<Tim4NoRemap, _, _>(
            geiger_boost_out,
            &mut afio.mapr,
            4200.Hz(),
            &clocks,
        );
        let mut geiger = Geiger::new(
            geiger_boost_pwm.split(),
            geiger_boost_feedback,
            geiger_out,
            adc1,
        );
        geiger.enable();
        critical_section::with(|cs| {
            if GEIGER.borrow(cs).set(RefCell::new(geiger)).is_err() {
                panic!("Failed to init GEIGER");
            }
        });
        unsafe {
            pac::NVIC::unmask(pac::Interrupt::EXTI9_5);
        }
    }

    loop {
        cortex_m::asm::wfi();
        critical_section::with(|cs| {
            GEIGER.borrow(cs).get().unwrap().borrow_mut().print_status();
        });
    }
}
