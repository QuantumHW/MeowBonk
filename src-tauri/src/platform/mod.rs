#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use std::sync::{atomic::AtomicBool, mpsc::SyncSender};
use std::sync::{Arc, Mutex};

#[cfg(debug_assertions)]
static COUNTERS: [std::sync::atomic::AtomicU64; 6] =
    [const { std::sync::atomic::AtomicU64::new(0) }; 6];
pub fn count(index: usize) {
    #[cfg(debug_assertions)]
    COUNTERS[index].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    #[cfg(not(debug_assertions))]
    let _ = index;
}
pub fn diagnostics() -> Option<[u64; 6]> {
    #[cfg(debug_assertions)]
    return Some(std::array::from_fn(|i| {
        COUNTERS[i].load(std::sync::atomic::Ordering::Relaxed)
    }));
    #[cfg(not(debug_assertions))]
    None
}

#[derive(Clone, Copy, Debug)]
pub struct Click {
    pub x: f64,
    pub y: f64,
}
pub struct Trigger {
    pub point: Click,
    pub created: std::time::Instant,
}
impl Trigger {
    pub fn new(point: Click) -> Self {
        Self {
            point,
            created: std::time::Instant::now(),
        }
    }
    pub fn fresh(&self) -> bool {
        self.created.elapsed() < std::time::Duration::from_millis(900)
    }
}
#[derive(Clone, Copy)]
pub struct InputRegion {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}
impl InputRegion {
    pub fn contains(&self, point: Click) -> bool {
        point.x.is_finite()
            && point.y.is_finite()
            && point.x >= self.left
            && point.y >= self.top
            && point.x < self.left + self.width
            && point.y < self.top + self.height
    }
}
#[derive(Clone, Copy)]
pub enum PointerPhase {
    Down,
    Up,
    Drag,
    Other,
}
#[derive(Default)]
pub struct CapturePolicy {
    captured: bool,
}
#[derive(Debug, PartialEq)]
pub struct Decision {
    pub consume: bool,
    pub trigger: bool,
}
impl CapturePolicy {
    pub fn handle(&mut self, phase: PointerPhase, modifiers: bool, in_scope: bool) -> Decision {
        let mut trigger = false;
        let consume = match phase {
            PointerPhase::Down if self.captured => true,
            PointerPhase::Down if modifiers && in_scope => {
                self.captured = true;
                trigger = true;
                true
            }
            PointerPhase::Up if self.captured => {
                self.captured = false;
                true
            }
            PointerPhase::Drag if self.captured => true,
            _ => false,
        };
        Decision { consume, trigger }
    }
}
pub type Status = Arc<Mutex<String>>;
pub fn report(status: &Status, value: impl Into<String>) {
    if let Ok(mut current) = status.lock() {
        *current = value.into();
    }
}

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "macos")]
pub use macos::{
    accessibility_permission, cursor, input_permission, permission, request_permission, run,
};
#[cfg(target_os = "windows")]
pub use windows::{
    accessibility_permission, cursor, input_permission, permission, request_permission, run,
};

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn permission() -> bool {
    false
}
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn request_permission() -> bool {
    false
}
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn run(_: SyncSender<Trigger>, _: Arc<AtomicBool>, status: Status, _: InputRegion) {
    report(&status, "此平台未实现输入监听");
}

pub fn local_point(
    click: Click,
    origin: (f64, f64),
    scale: f64,
    logical_input: bool,
    size: (f64, f64),
) -> Option<(f64, f64)> {
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    let (x, y) = if logical_input {
        (click.x - origin.0 / scale, click.y - origin.1 / scale)
    } else {
        ((click.x - origin.0) / scale, (click.y - origin.1) / scale)
    };
    (x.is_finite() && y.is_finite() && x >= 0.0 && y >= 0.0 && x < size.0 && y < size.1)
        .then_some((x, y))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn delayed_trigger_is_discarded_without_replaying_old_clicks() {
        let mut trigger = Trigger::new(Click { x: 20., y: 30. });
        assert!(trigger.fresh());
        trigger.created -= std::time::Duration::from_secs(1);
        assert!(!trigger.fresh());
    }
    #[test]
    fn captured_click_consumes_release_even_after_modifiers_are_released() {
        let mut policy = CapturePolicy::default();
        assert_eq!(
            policy.handle(PointerPhase::Down, true, true),
            Decision {
                consume: true,
                trigger: true
            }
        );
        assert_eq!(
            policy.handle(PointerPhase::Drag, false, false),
            Decision {
                consume: true,
                trigger: false
            }
        );
        assert_eq!(
            policy.handle(PointerPhase::Up, false, false),
            Decision {
                consume: true,
                trigger: false
            }
        );
        assert_eq!(
            policy.handle(PointerPhase::Down, false, true),
            Decision {
                consume: false,
                trigger: false
            }
        );
        assert_eq!(
            policy.handle(PointerPhase::Up, false, true),
            Decision {
                consume: false,
                trigger: false
            }
        );
    }
    #[test]
    fn ordinary_and_out_of_scope_clicks_are_not_consumed() {
        let mut policy = CapturePolicy::default();
        for phase in [
            PointerPhase::Down,
            PointerPhase::Drag,
            PointerPhase::Up,
            PointerPhase::Other,
        ] {
            assert_eq!(
                policy.handle(phase, false, true),
                Decision {
                    consume: false,
                    trigger: false
                }
            );
        }
        assert_eq!(
            policy.handle(PointerPhase::Down, true, false),
            Decision {
                consume: false,
                trigger: false
            }
        );
        assert_eq!(
            policy.handle(PointerPhase::Up, true, true),
            Decision {
                consume: false,
                trigger: false
            }
        );
    }
    #[test]
    fn windows_physical_coordinates_with_negative_origin() {
        assert_eq!(
            local_point(
                Click {
                    x: -1800.0,
                    y: 300.0
                },
                (-1920.0, 0.0),
                1.5,
                false,
                (1280.0, 720.0)
            ),
            Some((80.0, 200.0))
        );
    }
    #[test]
    fn mac_logical_coordinates_are_not_scaled_twice() {
        assert_eq!(
            local_point(
                Click { x: 200.0, y: 100.0 },
                (0.0, 0.0),
                2.0,
                true,
                (1512.0, 982.0)
            ),
            Some((200.0, 100.0))
        );
    }
    #[test]
    fn outside_and_invalid_points_are_rejected() {
        assert_eq!(
            local_point(
                Click { x: 100.0, y: 20.0 },
                (0.0, 0.0),
                1.0,
                false,
                (100.0, 100.0)
            ),
            None
        );
        assert_eq!(
            local_point(
                Click {
                    x: f64::NAN,
                    y: 20.0
                },
                (0.0, 0.0),
                1.0,
                false,
                (100.0, 100.0)
            ),
            None
        );
        assert_eq!(
            local_point(
                Click { x: 1.0, y: 1.0 },
                (0.0, 0.0),
                0.0,
                false,
                (100.0, 100.0)
            ),
            None
        );
    }
}
