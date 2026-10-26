//! Browser-independent floating-surface placement.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Top,
    Bottom,
    Left,
    Right,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub x: f64,
    pub y: f64,
    pub side: Side,
    pub max_height: f64,
}
pub fn place(anchor: Rect, size: Size, viewport: Rect, preferred: Side, offset: f64) -> Placement {
    let padding = 8.0;
    let left = viewport.x + padding;
    let top = viewport.y + padding;
    let right = viewport.x + viewport.width - padding;
    let bottom = viewport.y + viewport.height - padding;
    let above = (anchor.y - top - offset).max(0.0);
    let below = (bottom - anchor.y - anchor.height - offset).max(0.0);
    let before = (anchor.x - left - offset).max(0.0);
    let after = (right - anchor.x - anchor.width - offset).max(0.0);
    let side = match preferred {
        Side::Top if size.height > above && below > above => Side::Bottom,
        Side::Bottom if size.height > below && above > below => Side::Top,
        Side::Left if size.width > before && after > before => Side::Right,
        Side::Right if size.width > after && before > after => Side::Left,
        _ => preferred,
    };
    let (x, y) = match side {
        Side::Top => (anchor.x, anchor.y - size.height - offset),
        Side::Bottom => (anchor.x, anchor.y + anchor.height + offset),
        Side::Left => (anchor.x - size.width - offset, anchor.y),
        Side::Right => (anchor.x + anchor.width + offset, anchor.y),
    };
    let max_height = match side {
        Side::Top => above,
        Side::Bottom => below,
        _ => (bottom - top).max(0.0),
    };
    Placement {
        x: x.clamp(left, (right - size.width).max(left)),
        y: y.clamp(top, (bottom - size.height.min(max_height)).max(top)),
        side,
        max_height,
    }
}
pub fn local_rect(client: Rect, wrapper: Rect, scale_x: f64, scale_y: f64) -> Rect {
    let sx = if scale_x.abs() > f64::EPSILON {
        scale_x
    } else {
        1.0
    };
    let sy = if scale_y.abs() > f64::EPSILON {
        scale_y
    } else {
        1.0
    };
    Rect {
        x: (client.x - wrapper.x) / sx,
        y: (client.y - wrapper.y) / sy,
        width: client.width / sx,
        height: client.height / sy,
    }
}
pub fn bottom_sheet(viewport: Rect, max_height: f64) -> Rect {
    let height = (viewport.height * 0.55).min(max_height).max(0.0);
    Rect {
        x: viewport.x,
        y: viewport.y + viewport.height - height,
        width: viewport.width,
        height,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flips_and_clamps() {
        let p = place(
            Rect {
                x: 390.,
                y: 5.,
                width: 10.,
                height: 20.,
            },
            Size {
                width: 200.,
                height: 100.,
            },
            Rect {
                x: 0.,
                y: 0.,
                width: 400.,
                height: 300.,
            },
            Side::Top,
            8.,
        );
        assert_eq!(p.side, Side::Bottom);
        assert_eq!(p.x, 192.);
        assert_eq!(p.y, 33.);
    }
    #[test]
    fn independent_scale_axes() {
        assert_eq!(
            local_rect(
                Rect {
                    x: 30.,
                    y: 60.,
                    width: 40.,
                    height: 90.
                },
                Rect {
                    x: 10.,
                    y: 30.,
                    width: 0.,
                    height: 0.
                },
                2.,
                3.
            ),
            Rect {
                x: 10.,
                y: 10.,
                width: 20.,
                height: 30.
            }
        );
    }
    #[test]
    fn visual_viewport_sheet() {
        let sheet = bottom_sheet(
            Rect {
                x: 3.,
                y: 100.,
                width: 360.,
                height: 300.,
            },
            420.,
        );
        assert_eq!(sheet.y + sheet.height, 400.);
        assert!(sheet.height < 300.);
    }
}
