//! Blinks an LED
//!
//! This assumes that a LED is connected to pc13 as is the case on the blue pill board.
//!
//! Note: Without additional hardware, PC13 should not be used to drive an LED, see page 5.1.2 of
//! the reference manual for an explanation. This is not an issue on the blue pill.

#![deny(unsafe_code)]
#![no_main]
#![no_std]

use cortex_m::asm::wfi;
use cortex_m_rt::entry;
use cortex_m_semihosting::hprintln;
use panic_halt as _;
use stm32f1xx_hal::{pac, prelude::*};

mod geiger;

#[entry]
fn main() -> ! {
    hprintln!("Hello, world!");

    // Get access to the core peripherals from the cortex-m crate
    let cp = cortex_m::Peripherals::take().unwrap();
    // Get access to the device specific peripherals from the peripheral access crate
    let dp = pac::Peripherals::take().unwrap();

    let mut flash = dp.FLASH.constrain();
    let rcc = dp.RCC.constrain();
    let clocks = rcc.cfgr.freeze(&mut flash.acr);
    let mut afio = dp.AFIO.constrain();

    // Init Geiger Tube driver
    let mut gpiob = dp.GPIOB.split();
    // let geiger_out = gpiob.pb8.into_floating_input(&mut gpiob.crh);
    // let geiger_boost_out = gpiob.pb9.into_alternate_open_drain(&mut gpiob.crh);
    let mut geiger_boost_out = gpiob.pb9.into_push_pull_output(&mut gpiob.crh);
    // let mut geiger_boost_pwm = dp
    //     .TIM4
    //     .pwm_hz(geiger_boost_out, &mut afio.mapr, 1.kHz(), &clocks);
    
    geiger_boost_out.set_low();

    loop {
        wfi();
    }
}
