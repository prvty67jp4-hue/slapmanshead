//! Controllers (Xbox, PlayStation, Switch Pro, the Steam Deck's own): read with gilrs, turned
//! into game actions and into menu navigation. The layout follows the usual console shooter:
//!
//! | Control        | In a match                       | In menus                    |
//! |----------------|----------------------------------|-----------------------------|
//! | Left stick     | move                             | move between buttons        |
//! | Right stick    | look                             | scroll                      |
//! | RT             | shoot / slash                    |                             |
//! | LT             | guard / aim down sights          |                             |
//! | A / Cross      | jump, wall-run, wall-jump        | press the selected button   |
//! | B / Circle     | slide (hold)                     | back                        |
//! | X / Square     | reload                           |                             |
//! | Y / Triangle   | next weapon                      |                             |
//! | LB             | dash                             | previous tab                |
//! | RB             | kick                             | next tab                    |
//! | R3             | finisher                         |                             |
//! | L3             | dash (second button)             |                             |
//! | D-pad up       | use item                         | move between buttons        |
//! | D-pad left     | blade                            |                             |
//! | D-pad right    | swap guns                        |                             |
//! | D-pad down     | quick chat                       |                             |
//! | View / Share   | player list (hold)               |                             |
//! | Menu / Options | pause                            | back                        |

/// A button, as a bit in [`PadState::held`].
pub mod btn {
    pub const A: u32 = 1 << 0;
    pub const B: u32 = 1 << 1;
    pub const X: u32 = 1 << 2;
    pub const Y: u32 = 1 << 3;
    pub const LB: u32 = 1 << 4;
    pub const RB: u32 = 1 << 5;
    pub const LT: u32 = 1 << 6;
    pub const RT: u32 = 1 << 7;
    pub const VIEW: u32 = 1 << 8;
    pub const MENU: u32 = 1 << 9;
    pub const L3: u32 = 1 << 10;
    pub const R3: u32 = 1 << 11;
    pub const UP: u32 = 1 << 12;
    pub const DOWN: u32 = 1 << 13;
    pub const LEFT: u32 = 1 << 14;
    pub const RIGHT: u32 = 1 << 15;
}

/// Triggers count as pressed past this.
pub const TRIGGER: f32 = 0.45;
/// Sticks ignore drift inside this.
pub const DEADZONE: f32 = 0.16;

/// One frame of a controller.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PadState {
    pub connected: bool,
    /// Sticks, -1..1, up and right positive, deadzone already removed.
    pub lx: f32,
    pub ly: f32,
    pub rx: f32,
    pub ry: f32,
    /// Triggers, 0..1.
    pub lt: f32,
    pub rt: f32,
    /// Buttons held now, and the ones pressed since the last frame (see [`btn`]).
    pub held: u32,
    pub pressed: u32,
}

impl PadState {
    pub fn down(&self, b: u32) -> bool {
        self.held & b != 0
    }
    pub fn tapped(&self, b: u32) -> bool {
        self.pressed & b != 0
    }
    /// Anything touched this frame (to know the person is playing with the controller).
    pub fn used(&self) -> bool {
        self.pressed != 0 || self.lx.abs() + self.ly.abs() + self.rx.abs() + self.ry.abs() > 0.3 || self.lt > TRIGGER || self.rt > TRIGGER
    }
}

/// A stick with a round deadzone, rescaled so it still reaches full tilt smoothly.
pub fn stick(x: f32, y: f32) -> (f32, f32) {
    let (x, y) = (if x.is_finite() { x } else { 0.0 }, if y.is_finite() { y } else { 0.0 });
    let len = (x * x + y * y).sqrt();
    if len <= DEADZONE {
        return (0.0, 0.0);
    }
    let k = ((len - DEADZONE) / (1.0 - DEADZONE)).min(1.0) / len;
    (x * k, y * k)
}

/// How fast the right stick turns the view (radians a second) at `sens` 1: a curve, so small
/// tilts aim finely and full tilt turns quickly.
pub const LOOK_YAW: f32 = 3.6;
pub const LOOK_PITCH: f32 = 2.4;

/// The look change this frame (yaw, pitch) from the right stick. `slow` (0..1) eases it while
/// the crosshair is over an enemy (aim assist).
pub fn look(rx: f32, ry: f32, sens: f32, invert_y: bool, slow: f32, dt: f32) -> (f32, f32) {
    let curve = |v: f32| v.signum() * v.abs().powf(1.8);
    let k = sens * (1.0 - slow.clamp(0.0, 0.6)) * dt;
    let dy = curve(ry) * LOOK_PITCH * k;
    (curve(rx) * LOOK_YAW * k, if invert_y { -dy } else { dy })
}

/// Menu moves from a controller, sent to the menus as key presses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nav {
    Up,
    Down,
    Left,
    Right,
    /// Press the selected button (A).
    Press,
    /// Back out (B).
    Back,
    /// Pause / close the menu (Menu button).
    Menu,
    PrevTab,
    NextTab,
}

/// Turns held directions into menu moves: one at once, then repeating while held.
#[derive(Clone, Debug, Default)]
pub struct NavRepeat {
    dir: Option<Nav>,
    wait: f32,
}

pub const NAV_FIRST: f32 = 0.38;
pub const NAV_REPEAT: f32 = 0.11;

impl NavRepeat {
    pub fn step(&mut self, s: &PadState, dt: f32) -> Vec<Nav> {
        let mut out = Vec::new();
        // the d-pad, or the left stick pushed most of the way
        let dir = if s.down(btn::UP) || s.ly > 0.6 {
            Some(Nav::Up)
        } else if s.down(btn::DOWN) || s.ly < -0.6 {
            Some(Nav::Down)
        } else if s.down(btn::LEFT) || s.lx < -0.6 {
            Some(Nav::Left)
        } else if s.down(btn::RIGHT) || s.lx > 0.6 {
            Some(Nav::Right)
        } else {
            None
        };
        if dir != self.dir {
            self.dir = dir;
            self.wait = NAV_FIRST;
            out.extend(dir);
        } else if dir.is_some() {
            self.wait -= dt;
            if self.wait <= 0.0 {
                self.wait = NAV_REPEAT;
                out.extend(dir);
            }
        }
        for (b, n) in [(btn::A, Nav::Press), (btn::B, Nav::Back), (btn::MENU, Nav::Menu), (btn::LB, Nav::PrevTab), (btn::RB, Nav::NextTab)] {
            if s.tapped(b) {
                out.push(n);
            }
        }
        out
    }
}

/// The controller hardware (gilrs). None where the system offers no controllers.
pub struct Pads {
    gilrs: Option<gilrs::Gilrs>,
    /// The controller last used (any of them drives the game).
    active: Option<gilrs::GamepadId>,
    held: u32,
    pub name: Option<String>,
}

impl Pads {
    pub fn new() -> Pads {
        // tests and screenshots never read real controllers
        let off = std::env::var_os("DUEL_NO_PAD").is_some();
        let gilrs = if off { None } else { gilrs::Gilrs::new().ok() };
        Pads { gilrs, active: None, held: 0, name: None }
    }

    /// Read the controllers for this frame.
    pub fn poll(&mut self) -> PadState {
        use gilrs::{Axis, Button};
        let Some(g) = self.gilrs.as_mut() else { return PadState::default() };
        while let Some(ev) = g.next_event() {
            match ev.event {
                gilrs::EventType::Disconnected if Some(ev.id) == self.active => self.active = None,
                gilrs::EventType::ButtonPressed(..) | gilrs::EventType::Connected => self.active = Some(ev.id),
                gilrs::EventType::AxisChanged(_, v, _) if v.abs() > 0.5 => self.active = Some(ev.id),
                _ => {}
            }
        }
        let id = self.active.filter(|id| g.connected_gamepad(*id).is_some()).or_else(|| g.gamepads().next().map(|(id, _)| id));
        self.active = id;
        let Some(pad) = id.and_then(|id| g.connected_gamepad(id)) else {
            self.name = None;
            self.held = 0;
            return PadState::default();
        };
        self.name = Some(pad.name().to_string());
        let trig = |b: Button| pad.button_data(b).map_or(if pad.is_pressed(b) { 1.0 } else { 0.0 }, |d| d.value());
        let (lt, rt) = (trig(Button::LeftTrigger2), trig(Button::RightTrigger2));
        let (lx, ly) = stick(pad.value(Axis::LeftStickX), pad.value(Axis::LeftStickY));
        let (rx, ry) = stick(pad.value(Axis::RightStickX), pad.value(Axis::RightStickY));
        let mut held = 0;
        for (b, bit) in [
            (Button::South, btn::A),
            (Button::East, btn::B),
            (Button::West, btn::X),
            (Button::North, btn::Y),
            (Button::LeftTrigger, btn::LB),
            (Button::RightTrigger, btn::RB),
            (Button::Select, btn::VIEW),
            (Button::Start, btn::MENU),
            (Button::LeftThumb, btn::L3),
            (Button::RightThumb, btn::R3),
            (Button::DPadUp, btn::UP),
            (Button::DPadDown, btn::DOWN),
            (Button::DPadLeft, btn::LEFT),
            (Button::DPadRight, btn::RIGHT),
        ] {
            if pad.is_pressed(b) {
                held |= bit;
            }
        }
        if lt > TRIGGER {
            held |= btn::LT;
        }
        if rt > TRIGGER {
            held |= btn::RT;
        }
        let pressed = held & !self.held;
        self.held = held;
        PadState { connected: true, lx, ly, rx, ry, lt, rt, held, pressed }
    }
}

/// What a button is called on the controller in hand (for on-screen prompts).
pub fn button_name(bit: u32, pad_name: Option<&str>) -> &'static str {
    let n = pad_name.unwrap_or("").to_ascii_lowercase();
    let ps = n.contains("playstation") || n.contains("dualshock") || n.contains("dualsense") || n.contains("ps4") || n.contains("ps5") || n.contains("sony");
    match bit {
        btn::A => if ps { "Cross" } else { "A" },
        btn::B => if ps { "Circle" } else { "B" },
        btn::X => if ps { "Square" } else { "X" },
        btn::Y => if ps { "Triangle" } else { "Y" },
        btn::LB => if ps { "L1" } else { "LB" },
        btn::RB => if ps { "R1" } else { "RB" },
        btn::LT => if ps { "L2" } else { "LT" },
        btn::RT => if ps { "R2" } else { "RT" },
        btn::VIEW => if ps { "Share" } else { "View" },
        btn::MENU => if ps { "Options" } else { "Menu" },
        btn::L3 => "L3",
        btn::R3 => "R3",
        btn::UP => "D-pad up",
        btn::DOWN => "D-pad down",
        btn::LEFT => "D-pad left",
        btn::RIGHT => "D-pad right",
        _ => "?",
    }
}

/// Every gameplay action and the controller button for it (shown in Settings and How to Play).
pub const LAYOUT: [(&str, &str); 18] = [
    ("Move", "Left stick"),
    ("Look", "Right stick"),
    ("Shoot / slash", "RT"),
    ("Guard / aim down sights", "LT"),
    ("Jump / wall-run / wall-jump", "A"),
    ("Slide (hold, while running)", "B"),
    ("Reload", "X"),
    ("Next weapon", "Y"),
    ("Dash", "LB  or  L3"),
    ("Kick", "RB"),
    ("Finisher", "R3"),
    ("Use item", "D-pad up"),
    ("Blade", "D-pad left"),
    ("Swap guns", "D-pad right"),
    ("Quick chat", "D-pad down"),
    ("Player list (hold)", "View"),
    ("Pause", "Menu"),
    ("Menus", "stick / d-pad to move, A to press, B back, LB / RB tabs"),
];

/// Quick chat lines (D-pad down opens them; the stick or d-pad picks, A sends, B closes).
pub const QUICK_CHAT: [&str; 6] = ["gg", "Nice shot!", "Good fight", "Rematch?", "One more round", "Sorry!"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sticks_ignore_drift_and_still_reach_full_tilt() {
        assert_eq!(stick(0.1, -0.1), (0.0, 0.0));
        let (x, y) = stick(1.0, 0.0);
        assert!((x - 1.0).abs() < 1e-5 && y == 0.0);
        let (x, _) = stick(0.5, 0.0);
        assert!(x > 0.3 && x < 0.5, "rescaled past the deadzone: {x}");
        assert_eq!(stick(f32::NAN, 0.9).0, 0.0);
        // a small tilt aims finely, full tilt turns fast; aim assist slows it over an enemy
        let (slow, _) = look(0.3, 0.0, 1.0, false, 0.0, 0.1);
        let (fast, _) = look(1.0, 0.0, 1.0, false, 0.0, 0.1);
        assert!(fast > slow * 5.0, "{slow} vs {fast}");
        let (assisted, _) = look(1.0, 0.0, 1.0, false, 0.35, 0.1);
        assert!(assisted < fast && assisted > fast * 0.5);
        let (_, up) = look(0.0, 1.0, 1.0, false, 0.0, 0.1);
        let (_, inv) = look(0.0, 1.0, 1.0, true, 0.0, 0.1);
        assert!(up > 0.0 && inv < 0.0);
    }

    #[test]
    fn menu_moves_once_then_repeat_while_held() {
        let mut nav = NavRepeat::default();
        let down = PadState { connected: true, held: btn::DOWN, pressed: btn::DOWN, ..Default::default() };
        assert_eq!(nav.step(&down, 0.016), vec![Nav::Down]);
        let holding = PadState { pressed: 0, ..down };
        let mut moves = 0;
        for _ in 0..60 {
            moves += nav.step(&holding, 1.0 / 60.0).len();
        }
        // a second held: the first move, a pause, then a steady repeat
        let expect = ((1.0 - NAV_FIRST) / NAV_REPEAT) as usize;
        assert!(moves >= expect - 1 && moves <= expect + 1, "{moves} repeats");
        assert!(nav.step(&PadState::default(), 0.016).is_empty(), "let go: nothing");
        let stick_up = PadState { connected: true, ly: 0.9, ..Default::default() };
        assert_eq!(nav.step(&stick_up, 0.016), vec![Nav::Up]);
        let a = PadState { connected: true, held: btn::A | btn::RB, pressed: btn::A | btn::RB, ..Default::default() };
        assert_eq!(nav.step(&a, 0.016), vec![Nav::Press, Nav::NextTab]);
        assert_eq!(button_name(btn::A, Some("DualSense Wireless Controller")), "Cross");
        assert_eq!(button_name(btn::A, Some("Steam Deck")), "A");
    }
}
