//! the handheld's buttons and sticks from evdev, and what they are to the
//! title under the controls settings.

use std::collections::HashSet;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::sync::{Arc, Mutex};

use zakuro_core::services::hid::PadState;

use crate::settings::{Buttons, Settings, Speed, Stick};

pub const A: u16 = 305;
pub const B: u16 = 304;
pub const X: u16 = 307;
pub const Y: u16 = 308;
pub const L1: u16 = 310;
pub const R1: u16 = 311;
pub const L2: u16 = 312;
pub const R2: u16 = 313;
pub const SELECT: u16 = 704;
pub const START: u16 = 705;
pub const R3: u16 = 707;
pub const FN: u16 = 708;
pub const UP: u16 = 544;
pub const DOWN: u16 = 545;
pub const LEFT: u16 = 546;
pub const RIGHT: u16 = 547;
const BUTTONS: [(u16, PadState); 12] = [
    (A, PadState::A),
    (B, PadState::B),
    (X, PadState::X),
    (Y, PadState::Y),
    (L1, PadState::L),
    (R1, PadState::R),
    (SELECT, PadState::SELECT),
    (START, PadState::START),
    (UP, PadState::UP),
    (DOWN, PadState::DOWN),
    (LEFT, PadState::LEFT),
    (RIGHT, PadState::RIGHT),
];
/// a stick's travel, the most it reports.
const STICK: f32 = 1800.0;

#[derive(Default)]
pub struct Pad {
    pub keys: HashSet<u16>,
    pub axes: [i32; 8],
}

/// the handheld's gamepad device and its name.
pub fn gamepad() -> Option<(String, String)> {
    let devices = std::fs::read_to_string("/proc/bus/input/devices").ok()?;
    devices.split("\n\n").find_map(|block| {
        let name = block.lines().find_map(|line| line.strip_prefix("N: Name="))?;
        if !name.contains("Gamepad") && !name.contains("Controller") && !name.contains("Joypad") && !h700_pad(name) {
            return None;
        }
        let handlers = block.lines().find_map(|line| line.strip_prefix("H: Handlers="))?;
        let event = handlers.split_whitespace().find(|handler| handler.starts_with("event"))?;
        Some((format!("/dev/input/{event}"), name.to_owned()))
    })
}

/// Anbernic's H700 handhelds, which number their buttons another way and
/// give the d-pad as a hat.
fn h700_pad(name: &str) -> bool {
    name == "Deeplay-keys" || name == "ANBERNIC-keys" || (name.starts_with("Anbernic RG") && !name.contains("ARC"))
}

pub fn read_pad(path: String, name: String, pad: Arc<Mutex<Pad>>) {
    // they come in as the R36S's
    let h700 = h700_pad(&name);
    log::info!("buttons from {name}{}", if h700 { ", numbered as on Anbernic's H700 handhelds" } else { "" });
    std::thread::spawn(move || {
        let Ok(mut device) = std::fs::File::open(&path) else {
            log::error!("could not open {path}");
            return;
        };
        let ranges: [Option<(i32, i32)>; 6] = std::array::from_fn(|axis| h700.then(|| axis_range(&device, axis)).flatten());
        let mut event = [0u8; 24];
        while device.read_exact(&mut event).is_ok() {
            let kind = u16::from_ne_bytes([event[16], event[17]]);
            let code = u16::from_ne_bytes([event[18], event[19]]);
            let value = i32::from_ne_bytes([event[20], event[21], event[22], event[23]]);
            let mut pad = pad.lock().unwrap();
            match kind {
                1 => {
                    let code = if h700 { h700_button(code) } else { code };
                    if value == 0 {
                        pad.keys.remove(&code);
                    } else {
                        pad.keys.insert(code);
                    }
                }
                3 if h700 && matches!(code, 16 | 17) => {
                    let (less, more) = if code == 16 { (LEFT, RIGHT) } else { (UP, DOWN) };
                    pad.keys.remove(&less);
                    pad.keys.remove(&more);
                    if value != 0 {
                        pad.keys.insert(if value < 0 { less } else { more });
                    }
                }
                // the sticks on axes 2 and 3, 4 and 5, scaled to the R36S's travel
                3 if h700 => {
                    if let Some(axis) = [None, None, Some(0), Some(1), Some(3), Some(4)].get(code as usize).copied().flatten() {
                        pad.axes[axis] = match ranges[code as usize] {
                            Some((least, most)) => ((2 * value - least - most) as f32 / (most - least) as f32 * STICK) as i32,
                            None => value,
                        };
                    }
                }
                3 if (code as usize) < pad.axes.len() => pad.axes[code as usize] = value,
                _ => {}
            }
        }
    });
}

/// an H700 handheld's button as the R36S numbers it.
fn h700_button(code: u16) -> u16 {
    match code {
        304 => A,
        305 => B,
        306 => Y,
        308 => L1,
        309 => R1,
        310 => SELECT,
        311 => START,
        312 => FN,
        313 => 706,
        314 => L2,
        315 => R2,
        316 => R3,
        other => other,
    }
}

/// an axis's least and most, as the device gives them.
fn axis_range(device: &std::fs::File, axis: usize) -> Option<(i32, i32)> {
    let mut info = [0i32; 6];
    // SAFETY: EVIOCGABS fills the six numbers of the axis's input_absinfo
    let read = unsafe { libc::ioctl(device.as_raw_fd(), (0x8018_4540 + axis) as _, info.as_mut_ptr()) };
    (read >= 0 && info[2] > info[1]).then_some((info[1], info[2]))
}

/// ZAKURO_KEYS=tick:KEY+KEY,... holds the handheld's keys by name from each
/// tick of the frontend's loop, which goes on while the game is paused, for
/// trying the menu without a gamepad.
pub fn parse_keys(spec: &str) -> Vec<(u64, HashSet<u16>)> {
    let code = |name: &str| match name {
        "A" => Some(A),
        "B" => Some(B),
        "X" => Some(X),
        "Y" => Some(Y),
        "L1" => Some(L1),
        "R1" => Some(R1),
        "L2" => Some(L2),
        "R2" => Some(R2),
        "SELECT" => Some(SELECT),
        "START" => Some(START),
        "FN" => Some(FN),
        "UP" => Some(UP),
        "DOWN" => Some(DOWN),
        "LEFT" => Some(LEFT),
        "RIGHT" => Some(RIGHT),
        _ => None,
    };
    spec.split(',')
        .filter_map(|entry| {
            let (tick, keys) = entry.trim().split_once(':')?;
            Some((tick.parse().ok()?, keys.split('+').filter_map(|name| code(name.trim())).collect()))
        })
        .collect()
}

/// a stick's position from -1 to 1, nothing near the middle.
pub fn stick(value: i32) -> f32 {
    if value.abs() < 200 { 0.0 } else { (value as f32 / STICK).clamp(-1.0, 1.0) }
}

/// the title's buttons and circle pad for the keys held and the sticks.
pub fn controls(keys: &HashSet<u16>, axes: &[i32; 8], settings: &Settings) -> (PadState, f32, f32) {
    let mut buttons = PadState::empty();
    for (key, button) in BUTTONS {
        let key = match (settings.buttons, key) {
            (Buttons::Xbox, A) => B,
            (Buttons::Xbox, B) => A,
            (Buttons::Xbox, X) => Y,
            (Buttons::Xbox, Y) => X,
            _ => key,
        };
        if keys.contains(&key) {
            buttons |= button;
        }
    }
    let (x, y) = (stick(axes[0]), -stick(axes[1]));
    match settings.left_stick {
        Stick::CirclePad => (buttons, x, y),
        Stick::Dpad => {
            for (pushed, button) in [(x < -0.5, PadState::LEFT), (x > 0.5, PadState::RIGHT), (y > 0.5, PadState::UP), (y < -0.5, PadState::DOWN)] {
                if pushed {
                    buttons |= button;
                }
            }
            (buttons, 0.0, 0.0)
        }
    }
}

/// how far the touch cursor goes a frame with the right stick held over.
pub fn cursor_step(speed: Speed) -> f32 {
    match speed {
        Speed::Slow => 2.0,
        Speed::Normal => 4.0,
        Speed::Fast => 7.0,
    }
}
