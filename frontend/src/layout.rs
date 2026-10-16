//! where the screens go on a 640x480 panel, left, top, width and height.

use crate::settings::Layout;

pub fn areas(layout: Layout, swapped: bool) -> [Option<[i32; 4]>; 2] {
    let [first, second] = match layout {
        // the first screen 1.3 times over, the second smaller under it
        Layout::BigTop if swapped => [Some([112, 0, 416, 312]), Some([180, 312, 280, 168])],
        Layout::BigTop => [Some([60, 0, 520, 312]), Some([208, 312, 224, 168])],
        Layout::TopOnly if swapped => [Some([0, 0, 640, 480]), None],
        Layout::TopOnly => [Some([0, 48, 640, 384]), None],
        // both across the panel, eight ninths of their size
        Layout::SideBySide if swapped => [Some([0, 133, 284, 213]), Some([284, 133, 356, 213])],
        Layout::SideBySide => [Some([0, 133, 356, 213]), Some([356, 133, 284, 213])],
        Layout::Original if swapped => [Some([160, 0, 320, 240]), Some([120, 240, 400, 240])],
        Layout::Original => [Some([120, 0, 400, 240]), Some([160, 240, 320, 240])],
    };
    if swapped { [second, first] } else { [first, second] }
}
