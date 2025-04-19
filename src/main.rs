use esp_idf_hal::{
    gpio::{self, PinDriver, Pull, Pin},
    peripheral::Peripheral,
    peripherals::Peripherals,
    task::notification::Notification,
    timer::{self, TimerDriver},
};
use std::num::NonZeroU32;

const REFRESH_RATE: u64 = 64 /* * 300*/;

struct Charlieplex<'a> {
    //pins: Vec<PinDriver<'a, gpio::AnyIOPin, gpio::InputOutput>>,
    pins: Vec<i32>,
    low: Option<PinDriver<'a, gpio::AnyIOPin, gpio::Output>>,
    high: Option<PinDriver<'a, gpio::AnyIOPin, gpio::Output>>,
    floating: Vec<PinDriver<'a, gpio::AnyIOPin, gpio::Input>>,
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
    false, false, true, true, true, true, true, true,
    false, true, true, true, true, true, true, true,
    true, true, true, true, true, true, true, true,
    true, true, true, true, true, true, true, true,
    true, true, true, true, true, true, true, true,
    true, true, true, true, true, true, true, true,
    true, true, true, true, true, true, false, true,
    true, true, true, true, true, true, true, false,
];

impl<'a> Charlieplex<'a> {
    fn new(
        pins: impl IntoIterator<Item = gpio::AnyIOPin>,
        reference: &'a [bool],
        layout: (impl Into<usize>, impl Into<usize>),
    ) -> Self {
        let pins = pins.into_iter().collect::<Vec<_>>();

        let layout = (layout.0.into(), layout.1.into());

        if pins.len() != layout.0 + layout.1 || reference.len() != layout.0 * layout.1 {
            panic!("Number of pins does not match the layout dimensions.");
        }

        let (floating, pins): (Vec<PinDriver<'a, gpio::AnyIOPin, gpio::Input>>, Vec<i32>) = pins
            .into_iter()
            .map(|pin| {
                let mut pin = PinDriver::input(pin).unwrap();
                let num = pin.pin();
                pin.set_pull(Pull::Floating).unwrap();
                (pin, num)
            })
            .unzip();

        Charlieplex {
            pins,
            floating,
            low: None,
            high: None,
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
        if let Some(high) = self.high.take() {
            let mut high = high.into_input().unwrap();
            high.set_pull(Pull::Floating).unwrap();
            self.floating.insert(0, high);
        };

        if let Some(low) = self.low.take() {
            let mut low = low.into_input().unwrap();
            low.set_pull(Pull::Floating).unwrap();
            self.floating.insert(0, low);
        };
         

        self.index += 1;
        while !self.reference[self.index] {
            if self.index == self.layout.0 * self.layout.1 - 1 {
                self.index = 0;
            } else {
                self.index += 1;
            }
        }

        let high = self.pins[(self.index / self.layout.0) + self.layout.0];
        let low = self.pins[self.index % self.layout.0];

        let mut to_be_high = 0;
        let mut to_be_low = 0;
        for (i, pin) in self.floating.iter().enumerate() {
            if pin.pin() == high {
                to_be_high = i;
            } else if pin.pin() == low {
                to_be_low = i;
            }
            if to_be_high != 0 && to_be_low != 0 {
                break;
            }
        }

        let high = self.floating.remove(to_be_high);
        self.high = Some(high.into_output().unwrap());
        self.high.as_mut().unwrap().set_high().unwrap();

        let low = self.floating.remove(if to_be_high > to_be_low { to_be_low } else { to_be_low - 1});
        self.low = Some(low.into_output().unwrap());
        self.high.as_mut().unwrap().set_low().unwrap();

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
    timer_driver.enable(true).unwrap();

    let mut grid = Charlieplex::new(
        [
            peripherals.pins.gpio15.into(),
            peripherals.pins.gpio2.into(),
            peripherals.pins.gpio4.into(),
            peripherals.pins.gpio5.into(),
            peripherals.pins.gpio18.into(),
            peripherals.pins.gpio19.into(),
            peripherals.pins.gpio21.into(),
            peripherals.pins.gpio22.into(),
            peripherals.pins.gpio23.into(),
            peripherals.pins.gpio13.into(),
            peripherals.pins.gpio12.into(),
            peripherals.pins.gpio14.into(),
            peripherals.pins.gpio27.into(),
            peripherals.pins.gpio26.into(),
            peripherals.pins.gpio25.into(),
            peripherals.pins.gpio33.into(),
        ],
        &B,
        (8usize, 8usize),
    );

    loop {
        let bitset = notification.wait(esp_idf_hal::delay::BLOCK).unwrap();
        if bitset == NonZeroU32::new(0xbeef).unwrap() {
            grid.step();
        }
    }
}
