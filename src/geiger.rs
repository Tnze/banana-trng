use defmt::*;
use embassy_futures::join::join;
use embassy_stm32::{
    adc::{Adc, SampleTime, VREF_INT},
    exti::ExtiInput,
    gpio::{OutputType, Pull},
    peripherals::{ADC1, EXTI8, PB0, PB8, PB9, TIM4},
    time::Hertz,
    timer::{
        low_level::CountingMode,
        simple_pwm::{PwmPin, SimplePwm},
    },
};
use embassy_sync::pubsub::DynPublisher;
use embassy_time::{Duration, Instant, Ticker};
use pid::Pid;
use ringbuffer::{ConstGenericRingBuffer, RingBuffer};

const GEIGER_BACKGROUND_LEVEL: f32 = 25. / 60.; // 盖革管本底脉冲数 pulses/sec
const GEIGER_SENSITIVITY: f32 = 44.; // 盖革管灵敏度 CPS at 1 mR/h Co-60
const BED: f32 = 0.0778; // 香蕉等效剂量 1 Banana Equivalent Dose = 0.0778 µSv

#[embassy_executor::task]
pub(crate) async fn run(
    adc: Adc<'static, ADC1>,
    boost_fb_pin: PB0,
    boost_pwm_pin: PB9,
    boost_pwm_tim: TIM4,
    geiger_output_pin: PB8,
    geiger_output_exti: EXTI8,
    publisher: DynPublisher<'static, Message>,
) {
    join(
        run_boost(adc, boost_fb_pin, boost_pwm_pin, boost_pwm_tim),
        run_count(geiger_output_pin, geiger_output_exti, publisher),
    )
    .await;
}

async fn run_boost(
    mut adc: Adc<'static, ADC1>,
    mut boost_fb_pin: PB0,
    boost_pwm_pin: PB9,
    boost_pwm_tim: TIM4,
) {
    let mut boost_pwm = SimplePwm::new(
        boost_pwm_tim,
        None,
        None,
        None,
        Some(PwmPin::new_ch4(boost_pwm_pin, OutputType::PushPull)),
        Hertz::khz(5),
        CountingMode::EdgeAlignedUp,
    );
    let mut boost_pwm_channel = boost_pwm.ch4();
    boost_pwm_channel.set_duty_cycle(boost_pwm_channel.max_duty_cycle() / 2);
    boost_pwm_channel.enable();

    let mut boost_duty = 0.5;
    let mut pid = Pid::<f32>::new(380., 0.3);
    pid.p(0.0008, 0.1);
    pid.d(0.0001, 0.01);

    let mut vrefint = adc.enable_vref();
    let mut ticker = Ticker::every(Duration::from_millis(500));
    adc.set_sample_time(SampleTime::CYCLES239_5);
    loop {
        let v = adc.read(&mut boost_fb_pin).await;
        let vrefint_sample = adc.read(&mut vrefint).await;
        let sample_volt = sample_volt(v, vrefint_sample);
        let boost_volt = geiger_volt(sample_volt);

        let next = pid.next_control_output(boost_volt);
        boost_duty = (boost_duty + next.output).clamp(0.0, 0.9);
        let max_duty = boost_pwm_channel.max_duty_cycle() as f32;
        boost_pwm_channel.set_duty_cycle((max_duty * (1. - boost_duty)) as u16);

        ticker.next().await;
    }
}

fn sample_volt(v: u16, vref: u16) -> f32 {
    const VREF: f32 = VREF_INT as f32;
    v as f32 * VREF / vref as f32 / 1000.
}

fn geiger_volt(sample_volt: f32) -> f32 {
    const R1: f32 = 4.7e6;
    const R2: f32 = 24.9e3;
    sample_volt * (R1 / R2)
}

async fn run_count(
    geiger_output_pin: PB8,
    geiger_output_exti: EXTI8,
    publisher: DynPublisher<'static, Message>,
) {
    let mut geiger_output = ExtiInput::new(geiger_output_pin, geiger_output_exti, Pull::None);
    let mut history = ConstGenericRingBuffer::<_, 100>::new();
    let mut last = Instant::now();
    loop {
        geiger_output.wait_for_falling_edge().await;
        let now = Instant::now();
        let dur = now.saturating_duration_since(last);
        last = now;

        // The history buffer is full, or the oldest record is passed 1 minus.
        // Delete some of them.
        while history.is_full()
            || history
                .peek()
                .is_some_and(|x| now.duration_since(*x) > Duration::from_secs(4 * 60))
        {
            history.dequeue();
        }
        history.push(now);

        let mut cps = f32::NAN;
        let mut value = f32::NAN;
        if history.len() >= 2 {
            if let (Some(oldest), Some(latest)) = (history.front(), history.back()) {
                let duration = latest.duration_since(*oldest);
                let count = history.len();
                cps = count as f32 / (duration.as_millis() as f32 / 1000.);
                value = (cps - GEIGER_BACKGROUND_LEVEL) / GEIGER_SENSITIVITY; // mR/h
            }
        }
        let msg = Message {
            dur: dur.as_millis(),
            cpm: cps * 60.,
            val: value * 8.76,
        };
        info!(
            "dur: {} ms, cpm: {}, val: {} µSv/h = {} BED",
            msg.dur,
            msg.cpm,
            msg.val,
            msg.val / BED, // 1 mR ≈ 8.76 uSv
        );
        publisher.publish_immediate(msg);
    }
}

#[derive(Clone)]
pub(crate) struct Message {
    pub(crate) dur: u64,
    pub(crate) cpm: f32,
    pub(crate) val: f32,
}
