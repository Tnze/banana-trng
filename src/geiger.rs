use cortex_m_semihosting::hprintln;
use stm32f1xx_hal::{
    adc::Adc,
    gpio::{Analog, ExtiPin, Pin},
    pac::{ADC1, TIM4},
    prelude::*,
    timer::PwmChannel,
};

// static GEIGER_COUNT: AtomicU32 = AtomicU32::new(0);
// static GEIGER_OUT: Mutex<OnceCell<RefCell<Pin<'B', 8>>>> = Mutex::new(OnceCell::new());

pub(super) struct Geiger<const PWM_C: u8, const OUT_P: char, const OUT_N: u8> {
    boost_pwm: PwmChannel<TIM4, PWM_C>,
    boost_fb: Pin<'B', 0, Analog>,
    boost_out: Pin<OUT_P, OUT_N>,
    boost_adc: Adc<ADC1>,

    count: u32,
}

impl<const PWM_C: u8, const OUT_P: char, const OUT_N: u8> Geiger<PWM_C, OUT_P, OUT_N> {
    pub(super) fn new(
        boost_pwm: PwmChannel<TIM4, PWM_C>,
        boost_fb: Pin<'B', 0, Analog>,
        boost_out: Pin<OUT_P, OUT_N>,
        boost_adc: Adc<ADC1>,
    ) -> Self {
        Self {
            boost_pwm,
            boost_fb,
            boost_out,
            boost_adc,

            count: 0,
        }
    }

    pub(super) fn enable(&mut self) {
        let max = self.boost_pwm.get_max_duty();
        let duty = (max as f32 * (1. - 0.993)) as u16;
        self.boost_pwm.set_duty(duty);
        self.boost_pwm.enable();
        hprintln!("duty {}, max {}", self.boost_pwm.get_duty(), max);
    }

    pub(super) fn interrupt(&mut self) {
        if self.boost_out.check_interrupt() {
            self.boost_out.clear_interrupt_pending_bit();
            self.count += 1;
        }
    }

    pub(super) fn print_status(&mut self) {
        hprintln!("Volt: {:.02}V, Count: {}", self.read_voltage(), self.count);
    }

    fn read_voltage(&mut self) -> f32 {
        let data: u16 = self.boost_adc.read(&mut self.boost_fb).unwrap();
        let volt = data as f32 * 1200_f32 / self.boost_adc.read_vref() as f32;
        volt * 0.200_f32
    }
}
