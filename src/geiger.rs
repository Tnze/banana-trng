use cortex_m_semihosting::hprintln;
use embedded_hal::pwm::SetDutyCycle;
use pid::Pid;
use stm32f1xx_hal::{
    adc::Adc,
    gpio::{Edge, ExtiPin},
    hal_02::adc::Channel,
    pac::{ADC1, EXTI},
    prelude::*,
};

// static GEIGER_COUNT: AtomicU32 = AtomicU32::new(0);
// static GEIGER_OUT: Mutex<OnceCell<RefCell<Pin<'B', 8>>>> = Mutex::new(OnceCell::new());

pub struct Geiger<PWM, FB, OUT>
where
    PWM: SetDutyCycle,
    FB: Channel<ADC1, ID = u8>,
    OUT: ExtiPin,
{
    boost_pwm: PWM,
    boost_fb: FB,
    geiger_out: OUT,
    boost_adc: Adc<ADC1>,

    boost_pid: Pid<f32>,
    boost_duty: f32,

    count: u32,
}

impl<PWM, FB, OUT> Geiger<PWM, FB, OUT>
where
    PWM: SetDutyCycle,
    FB: Channel<ADC1, ID = u8>,
    OUT: ExtiPin,
{
    pub(super) fn new(boost_pwm: PWM, boost_fb: FB, boost_out: OUT, boost_adc: Adc<ADC1>) -> Self {
        let mut boost_pid = Pid::new(380., 0.1);
        boost_pid.p(0.001, 0.1);
        boost_pid.d(0.0001, 0.001);

        Self {
            boost_pwm,
            boost_fb,
            geiger_out: boost_out,
            boost_adc,

            boost_pid,
            boost_duty: 0.5,

            count: 0,
        }
    }

    pub(super) fn enable(&mut self, exti: &mut EXTI) {
        let max = self.boost_pwm.max_duty_cycle() as f32;
        let duty = (max * (1. - self.boost_duty)) as u16;
        self.boost_pwm.set_duty_cycle(duty).unwrap();
        self.boost_adc.enable_eoc_interrupt();
        self.interrupt_adc();

        // self.geiger_out.make_interrupt_source(afio);
        self.geiger_out.trigger_on_edge(exti, Edge::Falling);
        self.geiger_out.enable_interrupt(exti);
    }

    pub(super) fn interrupt_exti(&mut self, w: &mut impl embedded_io::Write) {
        if self.geiger_out.check_interrupt() {
            self.geiger_out.clear_interrupt_pending_bit();
            self.count += 1;
            let _ = writeln!(w, "Geiger Count: {}", self.count);
        }
    }

    pub(super) fn interrupt_adc(&mut self) {
        while let Result::<u16, _>::Ok(data) = self.boost_adc.read(&mut self.boost_fb) {
            let boost_volt = {
                let adc_volt = data as f32 * 1200_f32 / self.boost_adc.read_vref() as f32;
                const R1: f32 = 4.7e6;
                const R2: f32 = 10e3;
                adc_volt * (R1 / R2) / 1000.
            };

            let next = self.boost_pid.next_control_output(boost_volt);
            self.boost_duty = (self.boost_duty + next.output).clamp(0.0, 0.9);
            let max_duty = self.boost_pwm.max_duty_cycle() as f32;
            let next_duty = (max_duty * (1. - self.boost_duty)) as u16;
            self.boost_pwm.set_duty_cycle(next_duty).unwrap();

            hprintln!(
                "Boost Voltage: {:.01}, PWM duty: {:.04}",
                boost_volt,
                self.boost_duty,
            );
        }
        // else Err(nb::Error::WouldBlock)
    }

    pub(super) fn get_count(&self) -> u32 {
        self.count
    }
}
