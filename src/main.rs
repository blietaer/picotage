#![no_std]
#![no_main]
#![allow(unused_variables)] // only used for development
#![allow(unused_imports)] // only used for development

use defmt::*;
use defmt_rtt as _;
use panic_probe as _;

use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::block::ImageDef;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::peripherals::{PIN_1, PIN_4, PWM_SLICE2, UART0};
use embassy_rp::pwm::{Config, Pwm, SetDutyCycle};
use embassy_rp::uart::{BufferedInterruptHandler, BufferedUartRx, Config as UartConfig};
use embassy_rp::Peri;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::Timer;
use embedded_io_async::Read;
use static_cell::StaticCell;

bind_interrupts!(struct Irqs {
    UART0_IRQ => BufferedInterruptHandler<UART0>;
});

/// Raised by the UART task each time bytes were actually received
static RX_ACTIVITY: Signal<CriticalSectionRawMutex, ()> = Signal::new();

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("==================================");
    info!("    Package: {} v{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
    info!("==================================");

    let p = embassy_rp::init(Default::default());
    let mut led = Output::new(p.PIN_15, Level::Low);

    spawner.spawn(uart0_rx_task(p.UART0, p.PIN_1).expect("Failed creating UART0 RX task"));
    spawner.spawn(pwm_set_dutycycle(p.PWM_SLICE2, p.PIN_4).expect("Failed creating PWM task"));

    info!("All Set! Running async loop ...");

    // Blink the LED only when the UART task reports received data
    loop {
        RX_ACTIVITY.wait().await;
        led.set_high();
        Timer::after_millis(50).await;
        led.set_low();
        Timer::after_millis(50).await;
    }
}

const LINE_MAX: usize = 640; // longest DSMR line seen is ~570 bytes

/// Reads a DSMR P1 smart-meter stream on UART0 RX (GPIO1, physical pin #2, 115200 8N1)
/// and logs every CRLF-terminated line, including the final `!CCCC` CRC line.
#[embassy_executor::task]
async fn uart0_rx_task(uart: Peri<'static, UART0>, rx_pin: Peri<'static, PIN_1>) {
    static RX_BUF: StaticCell<[u8; 2048]> = StaticCell::new();
    let rx_buf = &mut RX_BUF.init([0; 2048])[..];

    let mut config = UartConfig::default();
    config.baudrate = 115_200;
    let mut rx = BufferedUartRx::new(uart, Irqs, rx_pin, rx_buf, config);

    let mut chunk = [0u8; 128];
    let mut line = heapless::Vec::<u8, LINE_MAX>::new();

    loop {
        let n = match rx.read(&mut chunk).await {
            Ok(n) => n,
            Err(e) => {
                error!("UART0 RX error: {:?}", e);
                continue;
            }
        };
        if n == 0 {
            continue;
        }
        RX_ACTIVITY.signal(());

        for &b in &chunk[..n] {
            if b == b'\n' {
                if line.last() == Some(&b'\r') {
                    line.pop();
                }
                if !line.is_empty() {
                    log_line(&line);
                }
                line.clear();
            } else if line.push(b).is_err() {
                warn!("UART0 RX line overflow, dropping {} bytes", line.len());
                line.clear();
            }
        }
    }
}

/// Human readable name of a DSMR OBIS code
fn obis_label(id: &str) -> Option<&'static str> {
    Some(match id {
        "0-0:96.1.4" => "DSMR version info",
        "0-0:96.1.1" => "Equipment identifier",
        "0-0:1.0.0" => "Timestamp",
        "1-0:1.8.1" => "Energy imported, tariff 1",
        "1-0:1.8.2" => "Energy imported, tariff 2",
        "1-0:2.8.1" => "Energy exported, tariff 1",
        "1-0:2.8.2" => "Energy exported, tariff 2",
        "0-0:96.14.0" => "Current tariff",
        "1-0:1.4.0" => "Current average demand",
        "1-0:1.6.0" => "Maximum demand this month",
        "0-0:98.1.0" => "Maximum demand history",
        "1-0:1.7.0" => "Power imported",
        "1-0:2.7.0" => "Power exported",
        "1-0:21.7.0" => "Power imported L1",
        "1-0:41.7.0" => "Power imported L2",
        "1-0:61.7.0" => "Power imported L3",
        "1-0:22.7.0" => "Power exported L1",
        "1-0:42.7.0" => "Power exported L2",
        "1-0:62.7.0" => "Power exported L3",
        "1-0:32.7.0" => "Voltage L1",
        "1-0:52.7.0" => "Voltage L2",
        "1-0:72.7.0" => "Voltage L3",
        "1-0:31.7.0" => "Current L1",
        "1-0:51.7.0" => "Current L2",
        "1-0:71.7.0" => "Current L3",
        "0-0:96.3.10" => "Breaker state",
        "0-0:17.0.0" => "Limiter threshold",
        "1-0:31.4.0" => "Fuse threshold L1",
        "0-1:96.3.10" | "0-2:96.3.10" | "0-3:96.3.10" | "0-4:96.3.10" => "Sub-meter switch state",
        "0-0:96.13.0" => "Text message",
        _ => return None,
    })
}

/// Decodes an ASCII hex string ("546F55" -> "ToU"); None if it isn't valid hex/ASCII
fn hex_to_text(hex: &str) -> Option<heapless::String<LINE_MAX>> {
    let mut out = heapless::String::new();
    for pair in hex.as_bytes().chunks(2) {
        let byte = u8::from_str_radix(core::str::from_utf8(pair).ok()?, 16).ok()?;
        if !(0x20..0x7F).contains(&byte) {
            return None;
        }
        out.push(byte as char).ok()?;
    }
    Some(out)
}

fn log_line(line: &[u8]) {
    // Always show text: drop the 8th bit (parity, if any) and replace non-printables by '?'
    let mut clean = heapless::String::<LINE_MAX>::new();
    for &b in line {
        let c = b & 0x7F;
        let _ = clean.push(if (0x20..0x7F).contains(&c) { c as char } else { '?' });
    }
    let text = clean.as_str();

    if let Some(ident) = text.strip_prefix('/') {
        info!("==== Meter telegram from {=str} ====", ident);
        return;
    }
    if let Some(crc) = text.strip_prefix('!') {
        info!("==== End of telegram (CRC {=str}) ====", crc);
        return;
    }

    // "OBIS(value[*unit])(value)..."
    let Some((id, rest)) = text.split_once('(') else {
        info!("{=str}", text);
        return;
    };
    let label = obis_label(id).unwrap_or(id);
    let rest = rest.strip_suffix(')').unwrap_or(rest);

    // Several value groups (e.g. maximum demand history): show them as-is
    if rest.contains(")(") {
        info!("{=str}: {=str}", label, rest);
        return;
    }

    match id {
        "0-0:1.0.0" if rest.len() == 13 => info!(
            "{=str}: 20{=str}-{=str}-{=str} {=str}:{=str}:{=str} ({=str})",
            label,
            &rest[0..2],
            &rest[2..4],
            &rest[4..6],
            &rest[6..8],
            &rest[8..10],
            &rest[10..12],
            if &rest[12..] == "S" { "summer time" } else { "winter time" }
        ),
        "0-0:96.1.1" | "0-0:96.13.0" => match hex_to_text(rest) {
            Some(t) => info!("{=str}: {=str}", label, t.as_str()),
            None => info!("{=str}: {=str}", label, rest),
        },
        _ => match rest.split_once('*') {
            // Drop leading zeros of numeric values: "015357.072" -> "15357.072"
            Some((value, unit)) => {
                let trimmed = value.trim_start_matches('0');
                let trimmed = if trimmed.is_empty() || trimmed.starts_with('.') {
                    &value[(value.len() - trimmed.len()).saturating_sub(1)..]
                } else {
                    trimmed
                };
                info!("{=str}: {=str} {=str}", label, trimmed, unit)
            }
            None => info!("{=str}: {=str}", label, rest),
        },
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


