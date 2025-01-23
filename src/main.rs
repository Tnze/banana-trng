#![deny(unsafe_code)]
// #![deny(warnings)]
#![no_main]
#![no_std]

use cortex_m_semihosting::hprintln;
// use panic_halt as _;
use panic_semihosting as _;

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
        usb::{Peripheral, UsbBus, UsbBusType},
    };
    use usb_device::prelude::*;

    use crate::geiger::Geiger;

    #[shared]
    struct Shared {
        geiger: Geiger<PwmChannel<TIM4, 3>, Pin<'B', 0, Analog>, Pin<'B', 8>>,
        usb_dev: UsbDevice<'static, UsbBusType>,
        serial: usbd_serial::SerialPort<'static, UsbBusType>,
    }

    #[local]
    struct Local {}

    #[init(local = [usb_bus: Option<usb_device::bus::UsbBusAllocator<UsbBusType>> = None])]
    fn init(mut ctx: init::Context) -> (Shared, Local) {
        hprintln!("hello, world");
        let mut afio = ctx.device.AFIO.constrain();
        let mut flash = ctx.device.FLASH.constrain();
        let rcc = ctx.device.RCC.constrain();
        let clocks = rcc
            .cfgr
            .use_hse(8.MHz())
            .sysclk(48.MHz())
            .pclk1(24.MHz())
            .freeze(&mut flash.acr);

        let gpioa = ctx.device.GPIOA.split();
        let mut gpiob = ctx.device.GPIOB.split();

        // Init USB
        let (usb_dev, serial) = {
            assert!(clocks.usbclk_valid());
            let pin_dm = gpioa.pa11;
            let pin_dp = gpioa.pa12;

            let usb = Peripheral {
                usb: ctx.device.USB,
                pin_dm,
                pin_dp,
            };
            ctx.local.usb_bus.replace(UsbBus::new(usb));
            let serial = usbd_serial::SerialPort::new(ctx.local.usb_bus.as_ref().unwrap());
            let usb_dev = UsbDeviceBuilder::new(
                ctx.local.usb_bus.as_ref().unwrap(),
                UsbVidPid(0x16c0, 0x27dd),
            )
            .device_class(usbd_serial::USB_CLASS_CDC)
            .strings(&[StringDescriptors::default()
                .manufacturer("Fake Company")
                .product("Serial port")
                .serial_number("TEST")])
            .unwrap()
            .build();

            (usb_dev, serial)
        };

        // Init Geiger Tube driver
        hprintln!("Init Geiger");
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

        (
            Shared {
                geiger,
                usb_dev,
                serial,
            },
            Local {},
        )
    }

    #[idle]
    fn idle(ctx: idle::Context) -> ! {
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
        ctx.shared.geiger.lock(|x| x.interrupt_adc());
    }

    #[task(binds = USB_HP_CAN_TX, shared = [usb_dev, serial])]
    fn usb_tx(ctx: usb_tx::Context) {
        hprintln!("USB_HP_CAN_TX");
        let mut usb_dev = ctx.shared.usb_dev;
        let mut serial = ctx.shared.serial;
        (&mut usb_dev, &mut serial).lock(|usb_dev, serial| {
            super::usb_poll(usb_dev, serial);
        });
    }

    #[task(binds = USB_LP_CAN_RX0, shared = [usb_dev, serial])]
    fn usb_rx0(ctx: usb_rx0::Context) {
        hprintln!("USB_LP_CAN_RX0");
        let mut usb_dev = ctx.shared.usb_dev;
        let mut serial = ctx.shared.serial;
        (&mut usb_dev, &mut serial).lock(|usb_dev, serial| {
            super::usb_poll(usb_dev, serial);
        });
    }
}

fn usb_poll<B: usb_device::bus::UsbBus>(
    usb_dev: &mut usb_device::prelude::UsbDevice<'static, B>,
    serial: &mut usbd_serial::SerialPort<'static, B>,
) {
    if !usb_dev.poll(&mut [serial]) {
        hprintln!("usb_polling failed");
        return;
    }
    hprintln!("usb_polled");

    let mut buf = [0u8; 8];

    match serial.read(&mut buf) {
        Ok(count) if count > 0 => {
            // Echo back in upper case
            for c in buf[0..count].iter_mut() {
                if 0x61 <= *c && *c <= 0x7a {
                    *c &= !0x20;
                }
            }

            serial.write(&buf[0..count]).ok();
        }
        _ => {}
    }
}
