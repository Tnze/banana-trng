use stm32f1xx_hal::{
    pac::TIM4,
    timer::{Pins, PwmHz, Remap},
};

pub(super) struct Geiger<REMAP, P, PINS>
where
    REMAP: Remap<Periph = TIM4>,
    PINS: Pins<REMAP, P>,
{
    boost_pwm: PwmHz<TIM4, REMAP, P, PINS>,
}
