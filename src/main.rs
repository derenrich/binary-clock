
#![no_std]
#![no_main]

use fugit::RateExtU32;
use bsp::entry;
use defmt::*;
use defmt_rtt as _;
use embedded_hal::digital::OutputPin;
use embedded_hal::digital::InputPin;
use panic_probe as _;
use ds323x::Ds323x;
use ds323x::{NaiveDate, DateTimeAccess, Timelike};
use ds323x::Datelike;
use is31fl3236::{IS31FL3236, Is31fl32xx, SoftwareShutdownMode, OutputCurrent, OutputMode, GlobalEnable};

// Provide an alias for our BSP so we can switch targets quickly.
// Uncomment the BSP you included in Cargo.toml, the rest of the code does not need to change.
use rp_pico as bsp;
// use sparkfun_pro_micro_rp2040 as bsp;

use bsp::hal::{
    clocks::{init_clocks_and_plls, Clock},
    i2c::I2C,
    pac,
    pac::interrupt,
    sio::Sio,
    watchdog::Watchdog,
    gpio,
    gpio::{FunctionI2c, PullNone, PullUp},
    gpio::Interrupt::EdgeLow,
};
use core::cell::RefCell;
use critical_section::Mutex;


type SquareWavePin = gpio::Pin<gpio::bank0::Gpio6, gpio::FunctionSioInput, PullUp>;

static square_wave: Mutex<RefCell<Option<SquareWavePin>>> = Mutex::new(RefCell::new(None));

#[entry]
fn main() -> ! {
    info!("Program start");
    let mut pac = pac::Peripherals::take().unwrap();
    let core = pac::CorePeripherals::take().unwrap();
    let mut watchdog = Watchdog::new(pac.WATCHDOG);
    let sio = Sio::new(pac.SIO);

    // External high-speed crystal on the pico board is 12Mhz
    let external_xtal_freq_hz = 12_000_000u32;
    let clocks = init_clocks_and_plls(
        external_xtal_freq_hz,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    let mut delay = cortex_m::delay::Delay::new(core.SYST, clocks.system_clock.freq().to_Hz());

    let pins = bsp::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    let mut sw = pins.gpio6.reconfigure::<gpio::FunctionSioInput, PullUp>();
    /*
    sw.set_interrupt_enabled(EdgeLow, true);

    critical_section::with(|cs| {
      square_wave.borrow(cs).replace(Some(sw));
    });
    unsafe {
        pac::NVIC::unmask(pac::Interrupt::IO_IRQ_BANK0);
    }*/


    let i2c = I2C::i2c0_with_external_pull_up(
        pac.I2C0,
    	pins.gpio0.reconfigure::<FunctionI2c, PullNone>(), // sda
    	pins.gpio1.reconfigure::<FunctionI2c, PullNone>(), // scl
    	400.kHz(),
    	&mut pac.RESETS,
    	125_000_000.Hz(),
    );

    let bus = shared_bus::BusManagerSimple::new(i2c);
    let mut rtc = Ds323x::new_ds3231(bus.acquire_i2c());
    rtc.set_square_wave_frequency(ds323x::SqWFreq::_1Hz).unwrap();
    rtc.enable_square_wave().unwrap();

    let mut ledC = Is31fl32xx::<IS31FL3236, _>::init_with_i2c(0x00, bus.acquire_i2c());
    ledC.set_shutdown(SoftwareShutdownMode::Normal).unwrap();

    ledC.set_global_output(GlobalEnable::Enable).unwrap();

    for n in 0..36 {
        info!("lighting: {}", n);
        ledC.set_led(n, OutputCurrent::IMaxDiv3, OutputMode::LEDOn).unwrap();
    }

    let mut b: u16 = 0;
    let mut led_pin = pins.led.into_push_pull_output();
    info!("loop");
    loop {
        //info!("on!");
        led_pin.set_high().unwrap();
        delay.delay_ms(20);
        //info!("off!");
        led_pin.set_low().unwrap();
        delay.delay_ms(20);
	let dt = rtc.datetime().unwrap();
	//info!("secs: {}", dt.num_seconds_from_midnight());
	for n in 0..36 {
          ledC.set_pwm(n, b as u8).unwrap();
       }
       b = (b + 1) % 255;
       
       let mut is_low = sw.is_low().unwrap();
       critical_section::with(|cs| {
         //let sw = square_wave.borrow(cs).take();
         //let mut sw_v = sw.unwrap();
         //is_low = sw_v.is_low().unwrap();
         //square_wave.borrow(cs).replace(Some(sw_v));       
       });
       if b % 1 == 0 {
       if is_low {
         info!("low");
       } else {
       	 info!("high");
       }
       }
    }
    //rtc.destroy_ds3231();
}

/*
#[interrupt]
fn IO_IRQ_BANK0() {
  info!("tick");
  critical_section::with(|cs| {
    //let sw0 = square_wave.borrow(cs);
    //let sw = sw0.take();
    //let mut sw_v = sw.unwrap();
    //sw_v.clear_interrupt(EdgeLow);
    //sw0.replace(Some(sw_v));
  });
  info!("tock");
}
*/