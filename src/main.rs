#![no_std]
#![no_main]


use defmt::*;
use embassy_executor::Spawner;
use embassy_executor::Executor;

use embassy_rp::gpio;
use embassy_rp::gpio::Pull;
use embassy_rp::peripherals::{I2C0};
use embassy_rp::multicore::Stack;

use shared_bus::BusManagerSimple;

use static_cell::StaticCell;
use gpio::{Level, Output, Input};
use {defmt_rtt as _, panic_probe as _};
use embassy_rp::i2c::{self, I2c, InterruptHandler};
use ds323x::Ds323x;
use ds323x::{NaiveDateTime, DateTimeAccess, Timelike};
use ds323x::Datelike;

use is31fl3236::{IS31FL3236, Is31fl32xx, SoftwareShutdownMode, OutputCurrent, OutputMode, GlobalEnable};

embassy_rp::bind_interrupts!(struct Irqs {
    I2C0_IRQ => InterruptHandler<embassy_rp::peripherals::I2C0>;
});

static I2C_BUS: StaticCell<BusManagerSimple<I2c<'static, I2C0, i2c::Blocking>>> = StaticCell::new();

const BUILD_UNIX_EPOCH: &str = env!("BUILD_UNIX_EPOCH");
fn build_time() -> u64 {
    BUILD_UNIX_EPOCH.parse().unwrap()
}

static mut CORE1_STACK: Stack<4096> = Stack::new();
static EXECUTOR1: StaticCell<Executor> = StaticCell::new();


#[embassy_executor::task]
async fn core1_task(mut led: Output<'static>) {
    // blink the LED on core 1
    loop {
        led.toggle();

        for _ in 0..1_000_000 {
            cortex_m::asm::nop();
        }
    }
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let led: Output = Output::new(p.PIN_25, Level::Low);

    embassy_rp::multicore::spawn_core1(p.CORE1, unsafe { &mut CORE1_STACK }, move || {
        let executor1 = EXECUTOR1.init(Executor::new());
        executor1.run(|spawner| spawner.spawn(core1_task(led)).unwrap());
    });


    // GPIO 6 = PIN 9
    let mut one_hz = Input::new(p.PIN_6, Pull::Up);

    let config = embassy_rp::i2c::Config::default();
    let i2c = embassy_rp::i2c::I2c::new_blocking(p.I2C0, p.PIN_1, p.PIN_0, config);

    let bus = I2C_BUS.init(BusManagerSimple::new(i2c));


    let i2c_dev1 = bus.acquire_i2c();
    let mut rtc = Ds323x::new_ds3231(i2c_dev1);
    rtc.set_square_wave_frequency(ds323x::SqWFreq::_1Hz).unwrap();
    rtc.use_int_sqw_output_as_square_wave().unwrap();
    rtc.enable_square_wave().unwrap();

    let i2c_dev2 = bus.acquire_i2c();
    let mut led_c = Is31fl32xx::<IS31FL3236, _>::init_with_i2c(0x00, i2c_dev2);
    led_c.set_global_output(GlobalEnable::Enable).unwrap();
    led_c.set_shutdown(SoftwareShutdownMode::Normal).unwrap();


    info!("build time: {}", build_time());
    let dt = rtc.datetime().unwrap();
    if dt.year() < 2020 {
        info!("setting time");
        let dt = NaiveDateTime::from_timestamp(build_time() as i64, 0);
        rtc.set_datetime(&dt).unwrap();
    } else {
        info!("clock already set");
    }

    loop {
        one_hz.wait_for_falling_edge().await;
        let dt = rtc.datetime().unwrap();
        let temp = rtc.temperature().unwrap();
        let timestamp = dt.and_utc().timestamp();
        info!("time: {}:{}:{} at {} C", dt.hour(), dt.minute(), dt.second(), temp);

        // iterate over 32 LEDs
        for n in 0..36 {
            let output_mode = if timestamp % 2 == 0 {
                OutputMode::LEDOn
            } else {
                OutputMode::LEDOff
            };
            led_c.set_led(n, OutputCurrent::IMaxDiv3, output_mode).unwrap();
            led_c.set_pwm(n, 0x3f as u8).unwrap();
        }
    }
}
