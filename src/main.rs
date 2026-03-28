#![allow(incomplete_features)]
#![feature(generic_const_exprs)]

use esp_idf_hal::{
    gpio::{InterruptType, PinDriver, Pull},
    peripherals::Peripherals,
};
use esp_idf_sys::*;
use std::{
    collections::HashMap,
    sync::atomic::{AtomicU8, Ordering},
};

// Indicates which buttons have been selected
static BUTTON_FLAGS: AtomicU8 = AtomicU8::new(0);

// A struct to display a message on an LED matrix using charlieplexing
struct Charlieplex {
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
                gpio_config(&config);

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

        // Resetting a GPIO to high impedence mode is more technically accurate, but it takes too
        // long, which breaks the illusion of persistance of vision. This method operates off of
        // the LEDs being diodes and reverses the polarity for disabled pins.
        unsafe {
            gpio_set_level(self.high, 1);
            gpio_set_level(self.low, 0);
        }
    }
}

// Indicates whether we display the title of a message or its contents
#[derive(PartialEq)]
enum MessageState {
    Playing,
    Advertising,
}

// A message player for an N x M LED matrix
struct MessagePlayer<const N: usize, const M: usize>
where
    [(); N * (((M + N) & !N) + N)]:,
{
    messages: Vec<(String, String)>,
    message_index: usize,          // which message
    window_index: usize,           // which character
    window_internal_offset: usize, // offset within character
    state: MessageState,
    window: [bool; N * (((M + N) & !N) + N)], // sized to 1 character past the width of the board
    alphabet: HashMap<char, u16>,
    layout: (usize, usize), // window layout (NOT BOARD)
}

impl<const N: usize, const M: usize> MessagePlayer<N, M>
where
    [(); N * (((M + N) & !N) + N)]:,
{
    fn new(alphabet: HashMap<char, u16>, messages: Vec<(String, String)>) -> Self {
        Self {
            messages,
            message_index: 0,
            window_index: 0,
            window_internal_offset: 0,
            state: MessageState::Advertising,
            window: [false; N * (((M + N) & !N) + N)],
            layout: (N, ((M + N) & !N) + N),
            alphabet,
        }
    }

    fn rebuild_window(&mut self) {
        let msg: &[char] = &match self.state {
            MessageState::Advertising => &self.messages[self.message_index].0,
            MessageState::Playing => &self.messages[self.message_index].1,
        }
        .to_uppercase()
        .chars()
        .collect::<Vec<_>>()[self.window_index..=self.window_index + M / (N + 1)];
        // M / (N + 1) = number of characters that can be fit in a row

        let mut msg_bit_coded = [0u16; N * (((M + N) & !N) + N)];

        msg.iter().enumerate().for_each(|(i, char)| {
            msg_bit_coded[i] = *self.alphabet.get(char).unwrap();
        });

        for i in 0..self.layout.0 {
            for j in 0..self.layout.1 {
                self.window[i * self.layout.1 + j] = if j % (N + 1) == N {
                    // space between characters
                    false
                } else {
                    // j % (N + 1) gives (0, 1, 2)
                    // N - 1 - (j % (N + 1)) gives the offset within the group (2, 1, 0)
                    // (N - i - 1) * N shifts the base according to the row
                    msg_bit_coded[j / (N + 1)] & (1 << ((N - 1 - (j % (N + 1))) + (N - i - 1) * N))
                        != 0
                }
            }
        }
    }

    fn step_window(&mut self) {
        self.window_internal_offset += 1;

        if self.window_internal_offset + M > self.layout.1 {
            // We overflowed the board and need to regenerate our reference
            self.window_internal_offset = 0;
            self.window_index += 1;

            if self.window_index + M / (N + 1) // + 1 for the additional character handled by >=
                >= match self.state {
                    MessageState::Advertising => &self.messages[self.message_index].0,
                    MessageState::Playing => &self.messages[self.message_index].1,
                }
                .len()
            {
                // We overflowed the message and need to start over
                if self.state == MessageState::Playing {
                    self.state = MessageState::Advertising;
                }

                self.window_index = 0;
                self.window_internal_offset = 0;
            }

            self.rebuild_window();
        }
    }

    fn cut_window(&mut self) -> [bool; N * M] {
        let mut out = [false; N * M];

        for row in 0..N {
            let src_start = row * self.layout.1 + self.window_internal_offset;
            let dst_start = row * M;

            out[dst_start..dst_start + M].copy_from_slice(&self.window[src_start..src_start + 8]);
        }

        out
    }

    fn toggle_play(&mut self) {
        self.state = match self.state {
            MessageState::Advertising => MessageState::Playing,
            MessageState::Playing => MessageState::Advertising,
        };

        self.window_internal_offset = 0;
        self.window_index = 0;
        self.rebuild_window();
    }

    fn next(&mut self) {
        self.message_index += 1;

        if self.message_index >= self.messages.len() {
            self.message_index = 0;
        }

        self.state = MessageState::Advertising;

        self.window_internal_offset = 0;
        self.window_index = 0;
        self.rebuild_window();
    }

    fn previous(&mut self) {
        if self.message_index == 0 {
            self.message_index = self.messages.len();
        }

        self.message_index -= 1;

        self.state = MessageState::Advertising;

        self.window_internal_offset = 0;
        self.window_index = 0;
        self.rebuild_window();
    }
}

fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    unsafe {
        esp_idf_sys::esp_task_wdt_deinit();
    }

    let peripherals = Peripherals::take().unwrap();

    let mut high = PinDriver::output(peripherals.pins.gpio33).unwrap();
    high.set_high().unwrap();

    let mut play = PinDriver::input(peripherals.pins.gpio26).unwrap();
    play.set_pull(Pull::Down).unwrap();
    play.set_interrupt_type(InterruptType::PosEdge).unwrap();

    let mut next = PinDriver::input(peripherals.pins.gpio27).unwrap();
    next.set_pull(Pull::Down).unwrap();
    next.set_interrupt_type(InterruptType::PosEdge).unwrap();

    let mut previous = PinDriver::input(peripherals.pins.gpio25).unwrap();
    previous.set_pull(Pull::Down).unwrap();
    previous.set_interrupt_type(InterruptType::PosEdge).unwrap();

    unsafe {
        play.subscribe(|| {
            BUTTON_FLAGS.fetch_or(0b001, Ordering::Relaxed);
        })
        .unwrap();

        next.subscribe(|| {
            BUTTON_FLAGS.fetch_or(0b010, Ordering::Relaxed);
        })
        .unwrap();

        previous
            .subscribe(|| {
                BUTTON_FLAGS.fetch_or(0b100, Ordering::Relaxed);
            })
            .unwrap();
    }

    play.enable_interrupt().unwrap();
    next.enable_interrupt().unwrap();
    previous.enable_interrupt().unwrap();

    let mut grid = Charlieplex::new(
        [15, 2, 4, 5, 18, 19, 21, 22, 12, 13, 23],
        Vec::from([false; 24]),
        (8usize, 3usize),
    );
    let mut message_player = MessagePlayer::<3, 8>::new(
        HashMap::from([
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
            (' ', 0b000000000),
            (':', 0b010000010),
            ('.', 0b000000010),
            (',', 0b000010110),
            ('\'', 0b010010000),
            ('0', 0b111101111),
            ('1', 0b110010111),
            ('2', 0b110010011),
            ('3', 0b110011110),
            ('4', 0b101111001),
            ('5', 0b011010110),
            ('6', 0b100111111),
            ('7', 0b111001001),
            ('8', 0b111111111),
            ('9', 0b111111001),
        ]),
        vec![
            ("title 1".to_string(), "message 1".to_string()),
            ("title 2".to_string(), "message 2".to_string()),
            ("title 3".to_string(), "message 3".to_string()),
            ("title 4".to_string(), "message 4".to_string()),
        ],
    );

    message_player.rebuild_window();
    let mut reference = message_player.cut_window();

    loop {
        grid.reference(Vec::from(reference));

        for _ in 0..150000 {
            grid.step();
        }

        message_player.step_window();
        reference = message_player.cut_window();

        // Handle Input
        let flags = BUTTON_FLAGS.swap(0, Ordering::Relaxed);
        if flags == 0 {
            continue;
        }

        if flags & 0b001 != 0 {
            // toggle play
            message_player.toggle_play();
            play.enable_interrupt().unwrap();
        }

        if flags & 0b010 != 0 {
            // next
            message_player.next();
            next.enable_interrupt().unwrap();
        }

        if flags & 0b100 != 0 {
            // prev
            message_player.previous();
            previous.enable_interrupt().unwrap();
        }

        reference = message_player.cut_window();
    }
}
