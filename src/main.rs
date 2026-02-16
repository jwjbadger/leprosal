use esp_idf_hal::{
    delay::FreeRtos, gpio::{self, Pin, PinDriver, Pull}, peripheral::Peripheral, peripherals::Peripherals, task::notification::Notification, timer::{self, TimerDriver}
};
use esp_idf_sys::*;
use std::{collections::HashMap, num::NonZeroU32};

const BITSET: NonZeroU32 = NonZeroU32::new(0xbeef).unwrap();

const BLANK: [bool; 24] = [
        true, true, true, true, true, true, true, true,
        true, true, true, true, true, true, true, true,
        true, true, true, true, true, true, true, true,
        ];

struct Charlieplex {
    //pins: Vec<PinDriver<'a, gpio::AnyIOPin, gpio::InputOutput>>,
    pins: Vec<i32>,
    low: i32,
    high: i32,
    reference: Vec<bool>,
    layout: (usize, usize),
    index: usize,
}

impl Charlieplex {
    fn new(
        pins: impl IntoIterator<Item = i32>,
        reference: Vec<bool>,
        layout: (impl Into<usize>, impl Into<usize>),
    ) -> Self {
        let pins = pins.into_iter().collect::<Vec<_>>();

        let layout = (layout.0.into(), layout.1.into());

        if pins.len() != layout.0 + layout.1 || reference.len() != layout.0 * layout.1 {
            panic!("Number of pins does not match the layout dimensions.");
        }

        unsafe {
            let mut config = gpio_config_t {
                pin_bit_mask: 0, // bitmask for GPIO pin
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

    fn reference(&mut self, reference: Vec<bool>) {
        if reference.len() != self.layout.0 * self.layout.1 {
            panic!("Reference length does not match the layout dimensions.");
        }

        self.reference = reference;
    }

    fn step(&mut self) {
        if self.high > 0 && self.low > 0 {
            unsafe {
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
            gpio_set_level(self.high, 1);
            gpio_set_level(self.low, 0);
        }
    }
}

fn build_window(alphabet: &HashMap<char, u16>, msg: &[char]) -> [bool; 33] {
    if msg.len() != 3 {
        panic!("improperly sized message");
    }

    let msg_bit_coded = [alphabet.get(&msg[0]).unwrap(), alphabet.get(&msg[1]).unwrap(), alphabet.get(&msg[2]).unwrap()];

    [
        msg_bit_coded[0] & (1 << 8) != 0, msg_bit_coded[0] & (1 << 7) != 0, msg_bit_coded[0] & (1 << 6) != 0, false, msg_bit_coded[1] & (1 << 8) != 0, msg_bit_coded[1] & (1 << 7) != 0, msg_bit_coded[1] & (1 << 6) != 0, false, msg_bit_coded[2] & (1 << 8) != 0, msg_bit_coded[2] & (1 << 7) != 0, msg_bit_coded[2] & (1 << 6) != 0,   
        msg_bit_coded[0] & (1 << 5) != 0, msg_bit_coded[0] & (1 << 4) != 0, msg_bit_coded[0] & (1 << 3) != 0, false, msg_bit_coded[1] & (1 << 5) != 0, msg_bit_coded[1] & (1 << 4) != 0, msg_bit_coded[1] & (1 << 3) != 0, false, msg_bit_coded[2] & (1 << 5) != 0, msg_bit_coded[2] & (1 << 4) != 0, msg_bit_coded[2] & (1 << 3) != 0,   
        msg_bit_coded[0] & (1 << 2) != 0, msg_bit_coded[0] & (1 << 1) != 0, msg_bit_coded[0] & (1 << 0) != 0, false, msg_bit_coded[1] & (1 << 2) != 0, msg_bit_coded[1] & (1 << 1) != 0, msg_bit_coded[1] & (1 << 0) != 0, false, msg_bit_coded[2] & (1 << 2) != 0, msg_bit_coded[2] & (1 << 1) != 0, msg_bit_coded[2] & (1 << 0) != 0,   
    ]
}

fn cut_window(data: &[bool; 33], offset: usize) -> [bool; 24] {
    let mut out = [false; 24];

    for row in 0..3 {
        let src_start = row * 11 + offset;
        let dst_start = row * 8;

        out[dst_start..dst_start + 8]
            .copy_from_slice(&data[src_start..src_start + 8]);
    }

    out
}

fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    unsafe {
        esp_idf_sys::esp_task_wdt_deinit();
    }

    let alphabet: HashMap<char, u16> = HashMap::from([
        ('A', 0b010000101),
        ('B', 0b110011111),
        ('C', 0b111100111),
        ('D', 0b110101110),
        ('E', 0b111110111),
        ('F', 0b111110100),
        ('G', 0b110001111),
        ('H', 0b101111101),
        ('I', 0b010010010),
        ('J', 0b011001111),
        ('K', 0b101110101),
        ('L', 0b100100111),
        ('M', 0b111111101),
        ('N', 0b111101101),
        ('O', 0b111101111),
        ('P', 0b111101100),
        ('Q', 0b111101100),
        ('R', 0b110111101),
        ('S', 0b110111011),
        ('T', 0b111010010),
        ('U', 0b101101111),
        ('V', 0b101000010),
        ('W', 0b101111111),
        ('X', 0b101010101),
        ('Y', 0b101010010),
        ('Z', 0b111010111),
    ]);

    let message = String::from("abcdefghijklmnopqrstuvwxyz").to_uppercase().chars().collect::<Vec<_>>();

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

    timer_driver.set_alarm((timer_driver.tick_hz() * 3) / 4 ).unwrap();

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
        [15, 2, 4, 5, 18, 19, 21, 22, 12, 13, 23],
        Vec::from(BLANK),
        (8usize, 3usize),
    );

    let mut super_index: usize = 0;
    let mut index: usize = 0;

    let mut window = build_window(&alphabet, &message[super_index..super_index + 3]);

    loop {
        grid.reference(Vec::from(cut_window(&window, index)));

        for _ in 0..100000 {
            grid.step();
        }

        index += 1;

        if index > 3 {
            super_index += 1;
            if super_index > message.len() - 3 {
                for _ in 0..500000 {
                    grid.step();
                }

                super_index = 0;
            }

            index = 0;
            window = build_window(&alphabet, &message[super_index..super_index + 3]);
        }
    }
}
