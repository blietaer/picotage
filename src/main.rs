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
    let mut led = Output::new(p.PIN_25, Level::Low);

    info!("All Set! Running async loop ...");

    // General async loop to ensure entire program runs forever while
    // asynchronously working on other tasks
    loop {
           led.set_high();
        Timer::after_millis(500).await;

        led.set_low();
        Timer::after_millis(500).await;

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
        c"This example tests the RP Pico on board LED, connected to gpio 25"
    ),
    embassy_rp::binary_info::rp_cargo_version!(),
    embassy_rp::binary_info::rp_program_build_attribute!(),
];


