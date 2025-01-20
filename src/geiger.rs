use cortex_m_semihosting::hprintln;
use embedded_hal::pwm::SetDutyCycle;
use pid::Pid;
use stm32f1xx_hal::{
    adc::Adc,
    gpio::{Analog, ExtiPin, Pin},
    pac::ADC1,
    prelude::*,
};

// static GEIGER_COUNT: AtomicU32 = AtomicU32::new(0);
// static GEIGER_OUT: Mutex<OnceCell<RefCell<Pin<'B', 8>>>> = Mutex::new(OnceCell::new());

pub(super) struct Geiger<PWM, const OUT_P: char, const OUT_N: u8>
where
    PWM: SetDutyCycle,
{
    boost_pwm: PWM,
    boost_fb: Pin<'B', 0, Analog>,
    boost_out: Pin<OUT_P, OUT_N>,
    boost_adc: Adc<ADC1>,

    boost_pid: Pid<f32>,
    boost_duty: f32,

    count: u32,
}

impl<PWM, const OUT_P: char, const OUT_N: u8> Geiger<PWM, OUT_P, OUT_N>
where
    PWM: SetDutyCycle,
{
    pub(super) fn new(
        boost_pwm: PWM,
        boost_fb: Pin<'B', 0, Analog>,
        boost_out: Pin<OUT_P, OUT_N>,
        boost_adc: Adc<ADC1>,
    ) -> Self {
        let mut boost_pid = Pid::new(380., 0.1);
        boost_pid.p(0.001, 0.1);
        boost_pid.d(0.0001, 0.001);

        Self {
            boost_pwm,
            boost_fb,
            boost_out,
            boost_adc,

            boost_pid,
            boost_duty: 0.5,

            count: 0,
        }
    }

    pub(super) fn enable(&mut self) {
        let max = self.boost_pwm.max_duty_cycle() as f32;
        let duty = (max * (1. - self.boost_duty)) as u16;
        self.boost_pwm.set_duty_cycle(duty).unwrap();
        // self.boost_pwm.enable();
        hprintln!("duty {}, max {}", duty, max);
    }

    pub(super) fn interrupt(&mut self) {
        if self.boost_out.check_interrupt() {
            self.boost_out.clear_interrupt_pending_bit();
            self.count += 1;
        }
    }

    pub(super) fn print_status(&mut self) {
        let volt = self.read_voltage();
        let next = self.boost_pid.next_control_output(volt);
        self.boost_duty = (self.boost_duty + next.output).clamp(0.0, 0.9);
        let max_duty = self.boost_pwm.max_duty_cycle() as f32;
        let next_duty = (max_duty * (1. - self.boost_duty)) as u16;
        self.boost_pwm.set_duty_cycle(next_duty).unwrap();

        hprintln!(
            "Volt: {:.02}V, duty: {:.04}, Count: {}",
            volt,
            self.boost_duty,
            self.count
        );
    }

    fn read_voltage(&mut self) -> f32 {
        let data: u16 = self.boost_adc.read(&mut self.boost_fb).unwrap();
        let volt = data as f32 * 1200_f32 / self.boost_adc.read_vref() as f32;

        const R1: f32 = 4.7e6;
        const R2: f32 = 10e3;
        volt * (R1 / R2) / 1000.
    }
}
