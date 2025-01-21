//! Blinks an LED
//!
//! This assumes that a LED is connected to pc13 as is the case on the blue pill board.
//!
//! Note: Without additional hardware, PC13 should not be used to drive an LED, see page 5.1.2 of
//! the reference manual for an explanation. This is not an issue on the blue pill.

#![deny(unsafe_code)]
// #![deny(warnings)]
#![no_main]
#![no_std]

use panic_halt as _;

mod display;
mod geiger;

#[rtic::app(device = stm32f1xx_hal::pac)]
mod app {
    use cortex_m_semihosting::hprintln;
    use stm32f1xx_hal::{
        adc::Adc,
        gpio::{Analog, ExtiPin, Pin},
        pac::TIM4,
        prelude::*,
        timer::{PwmChannel, Tim4NoRemap},
    };

    use crate::geiger::Geiger;

    #[shared]
    struct Shared {
        geiger: Geiger<PwmChannel<TIM4, 3>, Pin<'B', 0, Analog>, Pin<'B', 8>>,
    }

    #[local]
    struct Local {}

    #[init]
    fn init(mut ctx: init::Context) -> (Shared, Local) {
        hprintln!("hello, world");
        let mut afio = ctx.device.AFIO.constrain();
        let mut flash = ctx.device.FLASH.constrain();
        let rcc = ctx.device.RCC.constrain();
        let clocks = rcc.cfgr.freeze(&mut flash.acr);
        let mut gpiob = ctx.device.GPIOB.split();

        // Init Geiger Tube driver
        let mut geiger = {
            let mut geiger_out = gpiob.pb8.into_floating_input(&mut gpiob.crh);
            geiger_out.make_interrupt_source(&mut afio);

            let geiger_boost_feedback = gpiob.pb0.into_analog(&mut gpiob.crl);
            let geiger_boost_out = gpiob.pb9.into_alternate_push_pull(&mut gpiob.crh);
            let mut geiger_boost_pwm = ctx
                .device
                .TIM4
                .pwm_hz::<Tim4NoRemap, _, _>(geiger_boost_out, &mut afio.mapr, 5.kHz(), &clocks)
                .split();
            geiger_boost_pwm.enable();

            let adc1 = Adc::adc1(ctx.device.ADC1, &clocks);
            Geiger::new(geiger_boost_pwm, geiger_boost_feedback, geiger_out, adc1)
        };
        geiger.enable(&mut ctx.device.EXTI);

        (Shared { geiger }, Local {})
    }

    #[idle]
    fn idle(_ctx: idle::Context) -> ! {
        loop {
            cortex_m::asm::wfi();
        }
    }

    #[task(binds = EXTI9_5, shared = [geiger])]
    fn geiger_signal(mut ctx: geiger_signal::Context) {
        ctx.shared.geiger.lock(|x| x.interrupt_exti());
    }

    #[task(binds = ADC1_2, shared = [geiger])]
    fn adc_eoc(mut ctx: adc_eoc::Context) {
        // hprintln!("ADC1_2 triggered");
        ctx.shared.geiger.lock(|x| x.interrupt_adc());
    }
}
