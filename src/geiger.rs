use cortex_m_semihosting::hprintln;
use stm32f1xx_hal::{
    pac::TIM4,
    timer::{Pins, PwmChannel, PwmHz, Remap},
};

const VIN: f32 = 5000.; // mV
const L: f32 = 15.; // mH
const ISTA: f32 = 100.; // mA
const TSTA: f32 = L * ISTA / VIN; // ms

pub(super) struct Geiger<const PWM_C: u8> {
    boost_pwm: PwmChannel<TIM4, PWM_C>,
}

impl<const PWM_C: u8> Geiger<PWM_C> {
    fn new(boost_pwm: PwmChannel<TIM4, PWM_C>) -> Self {
        Self { boost_pwm }
    }

    fn enable(&mut self) {
        self.boost_pwm.enable();
    }

    fn feedback(&mut self, mv: u16) {
        hprintln!("feedback: {}(mV)", mv);
    }
}
