use std::{fs::Metadata, os::windows::fs::MetadataExt};
#[path = "cpa.rs"]
pub(crate) mod cpa;
use tauri::{PhysicalPosition, PhysicalRect, Rect, WebviewWindow};

pub(crate) fn is_reparse_point(metadata: &Metadata) -> bool {
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

pub(crate) fn position_tray(window: &WebviewWindow, rect: Option<Rect>) -> Result<(), String> {
    let error = |_| "无法定位快捷面板".to_owned();
    let Some(rect) = rect else {
        return window.center().map_err(error);
    };
    let position = rect.position.to_physical::<i32>(1.0);
    let Some(monitor) = window
        .monitor_from_point(f64::from(position.x), f64::from(position.y))
        .map_err(error)?
    else {
        return window.center().map_err(error);
    };
    let scale = monitor.scale_factor();
    let icon = PhysicalRect {
        position: rect.position.to_physical(scale),
        size: rect.size.to_physical(scale),
    };
    let panel = tauri::LogicalSize::new(400., 270.).to_physical(scale);
    let gap = (6. * scale).round() as i32;
    window
        .set_position(tray_origin(*monitor.work_area(), icon, panel, gap))
        .map_err(error)
}

fn tray_origin(
    work: PhysicalRect<i32, u32>,
    icon: PhysicalRect<i32, u32>,
    panel: tauri::PhysicalSize<i32>,
    gap: i32,
) -> PhysicalPosition<i32> {
    let left = work.position.x;
    let top = work.position.y;
    let right = (left + work.size.width as i32 - panel.width).max(left);
    let bottom = (top + work.size.height as i32 - panel.height).max(top);
    let below = icon.position.y + icon.size.height as i32 + gap;
    let y = if below <= bottom {
        below
    } else {
        icon.position.y - panel.height - gap
    };
    PhysicalPosition::new(
        (icon.position.x + icon.size.width as i32 - panel.width).clamp(left, right),
        y.clamp(top, bottom),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::PhysicalSize;

    #[test]
    fn tray_opens_within_the_work_area_on_a_left_hand_monitor() {
        let work = PhysicalRect {
            position: PhysicalPosition::new(-1920, 0),
            size: PhysicalSize::new(1920, 1040),
        };
        let mut icon = PhysicalRect {
            position: PhysicalPosition::new(-80, 1050),
            size: PhysicalSize::new(16, 16),
        };
        let panel = PhysicalSize::new(400, 270);
        assert_eq!(
            tray_origin(work, icon, panel, 6),
            PhysicalPosition::new(-464, 770)
        );
        icon.position = PhysicalPosition::new(-1910, -19);
        assert_eq!(
            tray_origin(work, icon, panel, 6),
            PhysicalPosition::new(-1920, 3)
        );
    }
}
