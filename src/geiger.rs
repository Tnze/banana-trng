use core::f32;

use cortex_m_semihosting::hprintln;
use embedded_hal::pwm::SetDutyCycle;
use pid::Pid;
use ringbuffer::RingBuffer;
use stm32f1xx_hal::{
    adc::Adc,
    gpio::{Edge, ExtiPin},
    hal_02::adc::Channel,
    pac::{ADC1, EXTI},
    prelude::*,
    rtc::Rtc,
};

const GEIGER_BACKGROUND_LEVEL: f32 = 25. / 60.; // 盖革管本底脉冲数 pulses/sec
const GEIGER_SENSITIVITY: f32 = 44.; // 盖革管灵敏度 CPS at 1 mR/h Co-60

pub struct Geiger<PWM, FB, OUT>
where
    PWM: SetDutyCycle,
    FB: Channel<ADC1, ID = u8>,
    OUT: ExtiPin,
{
    boost_pwm: PWM,
    boost_fb: FB,
    geiger_out: OUT,
    geiger_rtc: Rtc,
    boost_adc: Adc<ADC1>,

    boost_pid: Pid<f32>,
    boost_duty: f32,

    count: u32,
    last_time: u32,
    history: ringbuffer::ConstGenericRingBuffer<u32, 100>,
}

impl<PWM, FB, OUT> Geiger<PWM, FB, OUT>
where
    PWM: SetDutyCycle,
    FB: Channel<ADC1, ID = u8>,
    OUT: ExtiPin,
{
    pub(super) fn new(
        boost_pwm: PWM,
        boost_fb: FB,
        geiger_out: OUT,
        geiger_rtc: Rtc,
        boost_adc: Adc<ADC1>,
    ) -> Self {
        let mut boost_pid = Pid::new(380., 0.1);
        boost_pid.p(0.001, 0.1);
        boost_pid.d(0.0001, 0.001);

        let last_time = geiger_rtc.current_time();
        Self {
            boost_pwm,
            boost_fb,
            geiger_out,
            geiger_rtc,
            boost_adc,

            boost_pid,
            boost_duty: 0.5,

            count: 0,
            last_time,
            history: ringbuffer::ConstGenericRingBuffer::new(),
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

            let t = self.geiger_rtc.current_time();
            let dt = t.wrapping_sub(self.last_time);
            self.last_time = t;

            // The history buffer is full, or the oldest record is passed 1 minus.
            // Delete some of them.
            while self.history.is_full()
                || self
                    .history
                    .peek()
                    .is_some_and(|x| t.wrapping_sub(*x) > 4 * 60_000)
            {
                self.history.dequeue();
            }
            self.history.push(t);

            let mut cps = f32::NAN;
            let mut value = f32::NAN;
            if self.history.len() >= 2 {
                if let (Some(oldest), Some(latest)) = (self.history.front(), self.history.back()) {
                    let duration = latest - oldest;
                    let count = self.history.len();
                    cps = count as f32 / (duration as f32 / 1000.);
                    value = (cps - GEIGER_BACKGROUND_LEVEL) / GEIGER_SENSITIVITY;
                }
            }
            let _ = writeln!(
                w,
                "count: {}, time: {} ms, cpm: {}, val: {} µR/h",
                self.count,
                dt,
                cps * 60.,
                value * 1000.,
            );
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
        }
        // else Err(nb::Error::WouldBlock)
    }
}
