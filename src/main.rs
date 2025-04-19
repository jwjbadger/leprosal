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

const BITSET: NonZeroU32 = NonZeroU32::new(0xbeef).unwrap();

#[rustfmt::skip]
const MESSAGE: [[bool; 64]; 5] = [
    [true, true, true, true, false, false, false, false,
    true, false, false, true, false, false, false, false,
    true, false, false, true, false, false, false, false,
    true, true, true, true, false, false, false, false,
    true, false, false, false, false, false, false, false,
    true, false, false, false, false, false, false, false,
    true, false, false, false, false, false, false, false,
    true, false, false, false, false, false, false, false,],

    [true, true, true, true, false, false, false, false,
    true, false, false, true, false, false, false, false,
    true, false, false, true, false, false, false, false,
    true, true, true, true, false, false, false, false,
    true, false, false, true, false, false, false, false,
    true, false, false, false, true, false, false, false,
    true, false, false, false, false, true, false, false,
    true, false, false, false, false, true, false, false,],

    [true, true, true, true, false, false, false, false,
    true, false, false, true, false, false, false, false,
    true, false, false, true, false, false, false, false,
    true, false, false, true, false, false, false, false,
    true, false, false, true, false, false, false, false,
    true, false, false, true, false, false, false, false,
    true, false, false, true, false, false, false, false,
    true, true, true, true, false, false, false, false,],

    [true, true, true, true, true, true, true, true,
    true, false, false, true, true, false, false, true,
    true, false, false, true, true, false, false, true,
    true, false, false, true, true, false, false, true,
    true, false, false, true, true, false, false, true,
    true, false, false, false, false, false, false, true,
    true, false, false, false, false, false, false, true,
    true, false, false, false, false, false, false, true,],

    [true, true, true, true, false, false, false, false,
    true, false, false, true, false, false, false, false,
    true, false, false, true, false, false, false, false,
    false, false, true, true, false, false, false, false,
    false, false, true, false, false, false, false, false,
    false, false, true, false, false, false, false, false,
    false, false, false, false, false, false, false, false,
    false, false, true, false, false, false, false, false,],
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
                mode: gpio_mode_t_GPIO_MODE_OUTPUT,
                pull_up_en: gpio_pullup_t_GPIO_PULLUP_DISABLE,
                pull_down_en: gpio_pulldown_t_GPIO_PULLDOWN_DISABLE,
                intr_type: gpio_int_type_t_GPIO_INTR_DISABLE,
            };

            for (i, pin) in pins.iter().enumerate() {
                config.pin_bit_mask = 1u64 << pin;
                gpio_config(&mut config);

                if i < 8 {
                    gpio_set_level(*pin, 1);
                } else {
                    gpio_set_level(*pin, 0);
                }
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
                /*let mut config = gpio_config_t {
                    pin_bit_mask: 1u64 << self.high,     // bitmask for GPIO pin
                    mode: gpio_mode_t_GPIO_MODE_INPUT,
                    pull_up_en: gpio_pullup_t_GPIO_PULLUP_DISABLE,
                    pull_down_en: gpio_pulldown_t_GPIO_PULLDOWN_DISABLE,
                    intr_type: gpio_int_type_t_GPIO_INTR_DISABLE,
                };

                gpio_config(&mut config);*/

                //config.pin_bit_mask = 1u64 << self.low;
                //gpio_config(&mut config);
                gpio_set_level(self.high, 0);
                gpio_set_level(self.low, 1);
            }
        }

        self.index += 1;
        while self.index > self.layout.0 * self.layout.1 - 1 || !self.reference[self.index] {
            if self.index >= self.layout.0 * self.layout.1 - 1 {
                self.index = 0;
            } else {
                self.index += 1;
            }
        }

        self.high = self.pins[(self.index / self.layout.0) + self.layout.0];
        self.low = self.pins[self.index % self.layout.0];

        unsafe {
            /*let mut config = gpio_config_t {
                pin_bit_mask: 1u64 << self.high,     // bitmask for GPIO pin
                mode: gpio_mode_t_GPIO_MODE_OUTPUT,
                pull_up_en: gpio_pullup_t_GPIO_PULLUP_DISABLE,
                pull_down_en: gpio_pulldown_t_GPIO_PULLDOWN_DISABLE,
                intr_type: gpio_int_type_t_GPIO_INTR_DISABLE,
            };

            gpio_config(&mut config);

            config.pin_bit_mask = 1u64 << self.low;
            gpio_config(&mut config);*/

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
    let mut timer_driver = TimerDriver::new(
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
        .set_alarm(timer_driver.tick_hz() * 2)
        .unwrap();

    unsafe {
        timer_driver
            .subscribe(move || {
                notifier.notify_and_yield(BITSET);
            })
        .unwrap();
        }

    timer_driver.enable_interrupt().unwrap();
    timer_driver.enable_alarm(true).unwrap();
    timer_driver.enable(true).unwrap();

    let mut grid = Charlieplex::new(
        [
        22,
        21,
        19,
        18,
        5,
        4,
        2,
        15,
        23,
        13,
        12,
        14,
        27,
        26,
        25,
        33,
        ],
        &MESSAGE[0],
        (8usize, 8usize),
        );

        let mut index: usize = 0;

        loop {
            let bitset = notification.wait(esp_idf_hal::delay::NON_BLOCK);
            //if bitset == NonZeroU32::new(0xbeef).unwrap() {
            match bitset {
                Some(BITSET) => {
                    if index == 4 {
                        index = 0;
                    } else {
                        index += 1;
                    }
                    grid.reference(&MESSAGE[index]);
                },
                None => {
                    grid.step();
                },
                _ => {
                    panic!("unexpected notification");
                }
            }
            //}
        }
}
