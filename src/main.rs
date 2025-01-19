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
    cell::{OnceCell, RefCell, SyncUnsafeCell},
    mem::MaybeUninit,
    sync::atomic::{AtomicU32, Ordering},
};

use cortex_m_rt::entry;
use cortex_m_semihosting::hprintln;
use critical_section::Mutex;
use panic_halt as _;
use stm32f1xx_hal::{
    adc,
    gpio::{Edge, ExtiPin, Pin},
    pac::{self, interrupt},
    prelude::*,
    timer::{Channel, Tim4NoRemap},
};

mod geiger;

static GEIGER_COUNT: AtomicU32 = AtomicU32::new(0);
static GEIGER_OUT: Mutex<OnceCell<RefCell<Pin<'B', 8>>>> = Mutex::new(OnceCell::new());

#[interrupt]
fn EXTI9_5() {
    critical_section::with(|cs| {
        if let Some(geiger_out) = GEIGER_OUT.borrow(cs).get() {
            let geiger_out = &mut *geiger_out.borrow_mut();
            if (*geiger_out).check_interrupt() {
                geiger_out.clear_interrupt_pending_bit();
                GEIGER_COUNT.fetch_add(1, Ordering::Release);
            }
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

    // Init Geiger Tube driver
    let mut gpiob = dp.GPIOB.split();

    // OUT
    let mut geiger_out = gpiob.pb8.into_floating_input(&mut gpiob.crh);
    geiger_out.make_interrupt_source(&mut afio);
    geiger_out.trigger_on_edge(&mut dp.EXTI, Edge::Falling);
    geiger_out.enable_interrupt(&mut dp.EXTI);
    critical_section::with(|cs| {
        if GEIGER_OUT.borrow(cs).set(RefCell::new(geiger_out)).is_err() {
            panic!("Failed to init GEIGER_OUT");
        }
    });
    unsafe {
        pac::NVIC::unmask(pac::Interrupt::EXTI9_5);
    }

    // BOOST
    let mut adc1 = adc::Adc::adc1(dp.ADC1, &clocks);
    let mut geiger_feedback = gpiob.pb0.into_analog(&mut gpiob.crl);

    // let mut geiger_boost_out = gpiob.pb9.into_push_pull_output(&mut gpiob.crh);
    // geiger_boost_out.set_high();

    let geiger_boost_out = gpiob.pb9.into_alternate_push_pull(&mut gpiob.crh);
    let mut geiger_boost_pwm =
        dp.TIM4
            .pwm_hz::<Tim4NoRemap, _, _>(geiger_boost_out, &mut afio.mapr, 8.kHz(), &clocks);

    hprintln!("Enable CH4");

    geiger_boost_pwm.set_period(4200.Hz());
    let max = geiger_boost_pwm.get_max_duty();
    let duty = (max as f32 * (1. - 0.993)) as u16;
    geiger_boost_pwm.set_duty(Channel::C4, duty);
    geiger_boost_pwm.enable(Channel::C4);

    hprintln!(
        "duty {}, max {}",
        geiger_boost_pwm.get_duty(Channel::C4),
        max
    );

    loop {
        let data: u16 = adc1.read(&mut geiger_feedback).unwrap();
        let volt = data as f32 * 1200_f32 / adc1.read_vref() as f32;
        let geiger_volt = volt * 0.200_f32;
        hprintln!("Volt: {:.02}V", geiger_volt);

        // hprintln!("wfi");
        cortex_m::asm::wfi();
        hprintln!("Geiger Count: {}", GEIGER_COUNT.load(Ordering::Acquire));
    }
}
