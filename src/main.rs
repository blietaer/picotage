#![no_std]
#![no_main]
#![allow(unused_variables)] // only used for development
#![allow(unused_imports)] // only used for development

use defmt::*;
use defmt_rtt as _;
use panic_probe as _;

use embassy_executor::Spawner;
use embassy_rp::config::Config as HalConfig;
use embassy_rp::gpio;
use embassy_rp::Peri;
use embassy_rp::peripherals::{PIN_4, PIN_25, PWM_SLICE2, PWM_SLICE4};
use embassy_rp::pwm::{Config, Pwm, SetDutyCycle};
use embassy_time::Timer;
use embassy_rp::block::ImageDef;
use gpio::{Level, Output};

mod state_machine;

#[embassy_executor::main]
// async fn main(_spawner: Spawner) {
async fn main(spawner: Spawner) {
    info!("==================================");
    info!("    Package: {} v{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
    info!("==================================");

   // Spawn state machine async task
    spawner
        .spawn(state_machine::state_machine_task())
        .expect("Failed spawning state machine");


    let p = embassy_rp::init(Default::default());
    let mut led = Output::new(p.PIN_15, Level::Low);
    spawner.spawn(pwm_set_dutycycle(p.PWM_SLICE2, p.PIN_4))
        .expect("Failed spawning PWM task");

    info!("All Set! Running async loop ...");

    // General async loop to ensure entire program runs forever while
    // asynchronously working on other tasks
    loop {
           led.set_high();
        Timer::after_millis(1000).await;

        led.set_low();
        Timer::after_millis(1000).await;

    }
}


/// Demonstrate PWM by setting duty cycle
///
/// Using GP4 in Slice2, make sure to use an appropriate resistor.
#[embassy_executor::task]
async fn pwm_set_dutycycle(slice2: Peri<'static, PWM_SLICE2>, pin4: Peri<'static, PIN_4>) {
    // If we aim for a specific frequency, here is how we can calculate the top value.
    // The top value sets the period of the PWM cycle, so a counter goes from 0 to top and then wraps around to 0.
    // Every such wraparound is one PWM cycle. So here is how we get 25KHz:
    let desired_freq_hz = 25_000;
    let clock_freq_hz = embassy_rp::clocks::clk_sys_freq();
    let divider = 16u8;
    let period = (clock_freq_hz / (desired_freq_hz * divider as u32)) as u16 - 1;

    let mut c = Config::default();
    c.top = period;
    c.divider = divider.into();

    let mut pwm = Pwm::new_output_a(slice2, pin4, c.clone());

    loop {
        // 100% duty cycle, fully on
        pwm.set_duty_cycle_fully_on().unwrap();
        Timer::after_secs(1).await;

        // 66% duty cycle. Expressed as simple percentage.
        pwm.set_duty_cycle_percent(66).unwrap();
        Timer::after_secs(1).await;

        // 25% duty cycle. Expressed as 32768/4 = 8192.
        pwm.set_duty_cycle(c.top / 4).unwrap();
        Timer::after_secs(1).await;

        // 0% duty cycle, fully off.
        pwm.set_duty_cycle_fully_off().unwrap();
        Timer::after_secs(1).await;
    }
}


// Loading configurations
// Note: This comes from the 'configs.rs' file created in 'build.rs' from 'configs.json'
// Note: All loaded configurations will be UPPERCASE string slice (&str) constants
include!(concat!(env!("OUT_DIR"), "/configs.rs"));

#[unsafe(link_section = ".start_block")]
#[used]
pub static IMAGE_DEF: ImageDef = ImageDef::secure_exe();



// Program metadata for `picotool info`.
// This isn't needed, but it's recomended to have these minimal entries.
#[unsafe(link_section = ".bi_entries")]
#[used]
pub static PICOTOOL_ENTRIES: [embassy_rp::binary_info::EntryAddr; 4] = [
    embassy_rp::binary_info::rp_program_name!(c"Picotage"),
    embassy_rp::binary_info::rp_program_description!(
        c"This example tests the RP Pico 2 Board "
    ),
    embassy_rp::binary_info::rp_cargo_version!(),
    embassy_rp::binary_info::rp_program_build_attribute!(),
];


