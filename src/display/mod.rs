#![allow(clippy::upper_case_acronyms)]
#![allow(clippy::option_map_unit_fn)]

mod driver;

use cortex_m::delay::Delay;
use display_interface_spi::SPIInterface;
use embedded_hal::PwmPin;
use heapless::mpmc::Q16;
use rp2040_hal::gpio::bank0::*;
use rp2040_hal::gpio::{Disabled, Pin, PullDown};
use rp2040_hal::multicore::{Multicore, Stack};
use rp2040_hal::pac::SPI1;
use rp2040_hal::pwm::{Channel, FreeRunning, Pwm3, A};
use rp2040_hal::spi::{Enabled, Spi};

const WIDTH: u16 = 240;
const HEIGHT: u16 = 135;
const SCREEN_SIZE: usize = (WIDTH * HEIGHT) as usize;

static mut CORE1_STACK: Stack<4096> = Stack::new();
static COMMAND_QUEUE: Q16<Command> = Q16::new();

type DC = Pin<Gpio16, Disabled<PullDown>>;
type CS = Pin<Gpio21, Disabled<PullDown>>;
type BL = Channel<Pwm3, FreeRunning, A>;
type RST = Pin<Gpio28, Disabled<PullDown>>;

type SPI = Spi<Enabled, SPI1, 8>;

pub enum Command {
    Clear,
    Splash,
    Home {
        layer_id: u8,
        layer_name: &'static str,
    }, 
    Selector {

    },
    Settings {

    },
}

pub struct Display {
    bl: BL,
}

impl Display {
    pub fn new<'mc>(
        dc: DC,
        cs: CS,
        rst: RST,
        mut bl: BL,
        spi: SPI,
        delay: &mut Delay,
        mc: &'mc mut Multicore<'mc>,
    ) -> Self {
        let rst = rst.into_push_pull_output();

        // Set backlight to zero initially
        bl.set_duty(0);

        // Setup SPI display_interface
        let dc = dc.into_push_pull_output();
        let cs = cs.into_push_pull_output();
        let interface = SPIInterface::new(spi, dc, cs);

        // Create display
        let display = mipidsi::Builder::st7789_pico1(interface)
            .with_display_size(WIDTH, HEIGHT)
            // We're using the display upside-down (so the jumper pins don't get in the way as much)
            .with_orientation(mipidsi::Orientation::LandscapeInverted(true))
            .init(delay, Some(rst))
            .unwrap();

        let cores = mc.cores();
        let core1 = &mut cores[1];

        // Spin up display controller on core1
        core1
            .spawn(unsafe { &mut CORE1_STACK.mem }, || driver::drive(display))
            .unwrap();

        Self { bl }
    }

    /// Sets the brightness of the display backlight.
    /// 
    /// Values lower than 0.0 or higher than 1.0 will be clamped to within that range.
    pub fn set_brightness(&mut self, brightness: f32) {
        let brightness = brightness.clamp(0.0, 1.0);
        let brightness = (u16::MAX as f32 * brightness) as u16;
        self.bl.set_duty(brightness);
    }

    /// Enqueue a command for the display.
    /// 
    /// Note: the queue can only hold 16 elements. If the queue is full,
    /// any excess will be silently dropped.
    pub fn send_command(&self, command: Command) {
        let _ = COMMAND_QUEUE.enqueue(command);
    }
}

