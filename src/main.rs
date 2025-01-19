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

use core::{
    cell::SyncUnsafeCell,
    mem::MaybeUninit,
    sync::atomic::{AtomicU32, Ordering},
};

use cortex_m_rt::entry;
use cortex_m_semihosting::hprintln;
use panic_halt as _;
use stm32f1xx_hal::{
    adc,
    gpio::{Edge, ExtiPin, Pin},
    pac::{self, interrupt},
    prelude::*, timer::{Channel, Tim4NoRemap},
};

mod geiger;

static GEIGER_COUNT: AtomicU32 = AtomicU32::new(0);
static mut GEIGER_OUT: SyncUnsafeCell<MaybeUninit<Pin<'B', 8>>> =
    SyncUnsafeCell::new(MaybeUninit::uninit());

#[interrupt]
fn EXTI9_5() {
    // Safety: interrupt mask is set after init GEIGER_OUT
    unsafe {
        let geiger_out = GEIGER_OUT.get_mut().assume_init_mut();
        if geiger_out.check_interrupt() {
            geiger_out.clear_interrupt_pending_bit();
            GEIGER_COUNT.fetch_add(1, Ordering::Acquire);
        }
    }
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

    // Init Geiger Tube driver
    let mut gpiob = dp.GPIOB.split();

    // OUT
    let mut geiger_out = gpiob.pb8.into_floating_input(&mut gpiob.crh);
    geiger_out.make_interrupt_source(&mut afio);
    geiger_out.trigger_on_edge(&mut dp.EXTI, Edge::Falling);
    geiger_out.enable_interrupt(&mut dp.EXTI);
    unsafe {
        *GEIGER_OUT.get() = MaybeUninit::new(geiger_out);
    }

    unsafe {
        pac::NVIC::unmask(pac::Interrupt::EXTI9_5);
    }

    // BOOST
    let mut adc1 = adc::Adc::adc1(dp.ADC1, &clocks);
    let mut geiger_feedback = gpiob.pb0.into_analog(&mut gpiob.crl);

    // let mut geiger_boost_out = gpiob.pb9.into_push_pull_output(&mut gpiob.crh);
    // geiger_boost_out.set_low();

    let geiger_boost_out = gpiob.pb9.into_alternate_push_pull(&mut gpiob.crh);
    let mut geiger_boost_pwm =
        dp.TIM4
            .pwm_hz::<Tim4NoRemap, _, _>(geiger_boost_out, &mut afio.mapr, 7.kHz(), &clocks);
    hprintln!("Enable CH4");
    geiger_boost_pwm.enable(Channel::C4);

    let max = geiger_boost_pwm.get_max_duty();
    geiger_boost_pwm.set_duty(Channel::C4, max * 13 / 100);
    let duty = geiger_boost_pwm.get_duty(Channel::C4);
    hprintln!("duty {}, max {}", duty, max);

    loop {
        let data: u16 = adc1.read(&mut geiger_feedback).unwrap();
        let volt = data as f32 * 1200_f32 / adc1.read_vref() as f32;
        hprintln!(
            "adc1 data: {}, volt: {:.02}(mV), geiger: {:.02}(V), count: {}",
            data,
            volt,
            volt * 0.200_f32,
            GEIGER_COUNT.load(Ordering::Relaxed)
        );
        // wfi();
    }
}
