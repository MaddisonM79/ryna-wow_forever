//! Where the window opens. Its size and position are remembered between runs (the window-state
//! plugin); before it shows, the remembered frame is checked against the monitor it would land on,
//! and one that no longer fits (a display unplugged, a smaller one) goes back to the default,
//! centred on the primary monitor.

use tauri::{LogicalSize, PhysicalPosition, PhysicalSize, WebviewWindow};

/// The default frame (`tauri.conf.json`'s `width` and `height`), in logical pixels.
pub const DEFAULT_WIDTH: f64 = 820.0;
pub const DEFAULT_HEIGHT: f64 = 560.0;

/// A rectangle in physical pixels: a window's outer frame, or a monitor's work area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i64,
    pub y: i64,
    pub width: i64,
    pub height: i64,
}

impl Rect {
    pub fn new(x: i64, y: i64, width: i64, height: i64) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    fn contains(&self, px: i64, py: i64) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }

    fn centre(&self) -> (i64, i64) {
        (self.x + self.width / 2, self.y + self.height / 2)
    }
}

/// Whether a window frame can stay where it was remembered: its centre is on one of the
/// monitors' work areas, and it is no larger than that work area. A frame that fails either goes
/// back to the default.
pub fn fits(frame: Rect, work_areas: &[Rect]) -> bool {
    let (cx, cy) = frame.centre();
    work_areas
        .iter()
        .find(|m| m.contains(cx, cy))
        .is_some_and(|m| frame.width <= m.width && frame.height <= m.height)
}

/// Check the restored frame against the monitors and reset it if it doesn't fit, then show the
/// window. Each step that fails is skipped: the window still shows, where the OS puts it.
pub fn place_and_show(window: &WebviewWindow) {
    if !restored_frame_fits(window) {
        reset(window);
    }
    let _ = window.show();
    let _ = window.set_focus();
}

fn restored_frame_fits(window: &WebviewWindow) -> bool {
    let (Ok(position), Ok(size)) = (window.outer_position(), window.outer_size()) else {
        // Nothing to check against: leave it where it is.
        return true;
    };
    let frame = Rect::new(
        position.x.into(),
        position.y.into(),
        size.width.into(),
        size.height.into(),
    );
    let Ok(monitors) = window.available_monitors() else {
        return true;
    };
    let work_areas: Vec<Rect> = monitors
        .iter()
        .map(|m| {
            let area = m.work_area();
            Rect::new(
                area.position.x.into(),
                area.position.y.into(),
                area.size.width.into(),
                area.size.height.into(),
            )
        })
        .collect();
    fits(frame, &work_areas)
}

/// The default size, centred on the primary monitor's work area.
fn reset(window: &WebviewWindow) {
    let _ = window.unmaximize();
    let _ = window.set_size(LogicalSize::new(DEFAULT_WIDTH, DEFAULT_HEIGHT));
    let Ok(Some(primary)) = window.primary_monitor() else {
        let _ = window.center();
        return;
    };
    let scale = primary.scale_factor();
    let size: PhysicalSize<u32> =
        LogicalSize::new(DEFAULT_WIDTH, DEFAULT_HEIGHT).to_physical(scale);
    let area = primary.work_area();
    let x = area.position.x + (area.size.width as i32 - size.width as i32) / 2;
    let y = area.position.y + (area.size.height as i32 - size.height as i32) / 2;
    let _ = window.set_position(PhysicalPosition::new(
        x.max(area.position.x),
        y.max(area.position.y),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAPTOP: Rect = Rect {
        x: 0,
        y: 25,
        width: 1512,
        height: 982 - 25,
    };
    const EXTERNAL: Rect = Rect {
        x: 1512,
        y: 0,
        width: 2560,
        height: 1440,
    };

    #[test]
    fn a_frame_on_a_monitor_fits() {
        assert!(fits(Rect::new(100, 100, 820, 560), &[LAPTOP]));
        assert!(fits(Rect::new(1600, 200, 1200, 900), &[LAPTOP, EXTERNAL]));
    }

    #[test]
    fn a_frame_half_off_the_edge_still_fits_while_its_centre_is_on_screen() {
        assert!(fits(Rect::new(1000, 600, 820, 560), &[LAPTOP]));
    }

    #[test]
    fn a_frame_whose_centre_is_on_no_monitor_does_not_fit() {
        // Where the external display used to be.
        assert!(!fits(Rect::new(1600, 200, 820, 560), &[LAPTOP]));
        assert!(!fits(Rect::new(-900, 100, 820, 560), &[LAPTOP]));
        assert!(!fits(Rect::new(100, 100, 820, 560), &[]));
    }

    #[test]
    fn a_frame_larger_than_the_work_area_does_not_fit() {
        // Sized for the external display, now centred on the laptop's.
        assert!(!fits(Rect::new(0, 25, 2000, 900), &[LAPTOP]));
        assert!(!fits(Rect::new(0, 25, 1512, 1000), &[LAPTOP]));
        // Exactly the work area is fine (a maximised window).
        assert!(fits(LAPTOP, &[LAPTOP]));
    }

    #[test]
    fn the_monitor_is_the_one_under_the_centre_not_the_corner() {
        // The top-left corner is on the laptop, the centre on the external: judged by the external.
        assert!(fits(Rect::new(1400, 100, 1800, 1200), &[LAPTOP, EXTERNAL]));
    }
}
