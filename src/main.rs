use esp_idf_hal::{
    gpio::{self, PinDriver, Pull, Pin},
    peripheral::Peripheral,
    peripherals::Peripherals,
    task::notification::Notification,
    timer::{self, TimerDriver},
};
use esp_idf_sys::*;
use std::num::NonZeroU32;

const REFRESH_RATE: u64 = 64 * 300;

struct Charlieplex<'a> {
    //pins: Vec<PinDriver<'a, gpio::AnyIOPin, gpio::InputOutput>>,
    pins: Vec<i32>,
    low: i32,
    high: i32,
    reference: &'a [bool],
    layout: (usize, usize),
    index: usize,
}

#[rustfmt::skip]
const A: [bool; 64] = [
    false, false, false, false, false, false, false, false,
    false, false, true, false, true, false, false, false,
    false, false, false, false, false, false, false, false,
    false, false, true, false, true, false, false, false,
    false, false, false, true, false, false, false, false,
    false, false, false, false, false, false, false, false,
    false, false, false, false, false, false, false, false,
    false, false, false, false, false, false, false, false,
];

#[rustfmt::skip]
const B: [bool; 64] = [
    true, true, true, true, false, false, false, false,
    true, false, false, true, false, false, false, false,
    true, false, false, true, false, false, false, false,
    true, true, true, true, false, false, false, false,
    true, false, false, false, false, false, false, false,
    true, false, false, false, false, false, false, false,
    true, false, false, false, false, false, false, false,
    true, false, false, false, false, false, false, false,
];

impl<'a> Charlieplex<'a> {
    fn new(
        pins: impl IntoIterator<Item = i32>,
        reference: &'a [bool],
        layout: (impl Into<usize>, impl Into<usize>),
    ) -> Self {
        let pins = pins.into_iter().collect::<Vec<_>>();

        let layout = (layout.0.into(), layout.1.into());

        if pins.len() != layout.0 + layout.1 || reference.len() != layout.0 * layout.1 {
            panic!("Number of pins does not match the layout dimensions.");
        }

        unsafe {
            let mut config = gpio_config_t {
                pin_bit_mask: 0,     // bitmask for GPIO pin
                mode: gpio_mode_t_GPIO_MODE_INPUT,
                pull_up_en: gpio_pullup_t_GPIO_PULLUP_DISABLE,
                pull_down_en: gpio_pulldown_t_GPIO_PULLDOWN_DISABLE,
                intr_type: gpio_int_type_t_GPIO_INTR_DISABLE,
            };

            for pin in pins.iter() {
                config.pin_bit_mask = 1u64 << pin;
                gpio_config(&mut config);
            }
        }

        Charlieplex {
            pins,
            low: -1,
            high: -1,
            layout,
            reference,
            index: 0,
        }
    }

    fn reference(&mut self, reference: &'a [bool]) {
        if reference.len() != self.layout.0 * self.layout.1 {
            panic!("Reference length does not match the layout dimensions.");
        }

        self.reference = reference;
    }

    fn step(&mut self) {
        if self.high > 0 && self.low > 0 {
            unsafe {
                let mut config = gpio_config_t {
                    pin_bit_mask: 1u64 << self.high,     // bitmask for GPIO pin
                    mode: gpio_mode_t_GPIO_MODE_INPUT,
                    pull_up_en: gpio_pullup_t_GPIO_PULLUP_DISABLE,
                    pull_down_en: gpio_pulldown_t_GPIO_PULLDOWN_DISABLE,
                    intr_type: gpio_int_type_t_GPIO_INTR_DISABLE,
                };

                gpio_config(&mut config);

                config.pin_bit_mask = 1u64 << self.low;
                gpio_config(&mut config);
            }
        }

        self.index += 1;
        while !self.reference[self.index] {
            if self.index == self.layout.0 * self.layout.1 - 1 {
                self.index = 0;
            } else {
                self.index += 1;
            }
        }

        self.high = self.pins[(self.index / self.layout.0) + self.layout.0];
        self.low = self.pins[self.index % self.layout.0];

        unsafe {
            let mut config = gpio_config_t {
                pin_bit_mask: 1u64 << self.high,     // bitmask for GPIO pin
                mode: gpio_mode_t_GPIO_MODE_OUTPUT,
                pull_up_en: gpio_pullup_t_GPIO_PULLUP_DISABLE,
                pull_down_en: gpio_pulldown_t_GPIO_PULLDOWN_DISABLE,
                intr_type: gpio_int_type_t_GPIO_INTR_DISABLE,
            };

            gpio_config(&mut config);

            config.pin_bit_mask = 1u64 << self.low;
            gpio_config(&mut config);

            gpio_set_level(self.high, 1);
            gpio_set_level(self.low, 0);
        }

        //println!("setting pin {} high and {} low", self.index % self.layout.0, (self.index / self.layout.0) + self.layout.0);
    }
}

fn main() {
    esp_idf_svc::sys::link_patches();
    //esp_idf_svc::log::EspLogger::initialize_default();

    unsafe {
        esp_idf_sys::esp_task_wdt_deinit();
    }

    let peripherals = Peripherals::take().unwrap();
    /*let mut timer_driver = TimerDriver::new(
        peripherals.timer00,
        &timer::config::Config {
            auto_reload: true,
            ..Default::default()
        },
    )
        .unwrap();

    let notification = Notification::new();
    let notifier = notification.notifier();

    timer_driver
        .set_alarm(timer_driver.tick_hz() / REFRESH_RATE)
        .unwrap();

    unsafe {
        timer_driver
            .subscribe(move || {
                notifier.notify_and_yield(NonZeroU32::new(0b1011111011101111).unwrap());
            })
        .unwrap();
        }

    timer_driver.enable_interrupt().unwrap();
    timer_driver.enable_alarm(true).unwrap();
    timer_driver.enable(true).unwrap();*/

    let mut grid = Charlieplex::new(
        [
        15,
        2,
        4,
        5,
        18,
        19,
        21,
        22,
        23,
        13,
        12,
        14,
        27,
        26,
        25,
        33,
        ],
        &B,
        (8usize, 8usize),
        );

        loop {
            //let bitset = notification.wait(esp_idf_hal::delay::BLOCK).unwrap();
            //if bitset == NonZeroU32::new(0xbeef).unwrap() {
                grid.step();
            //}
        }
}
