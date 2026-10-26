//! World geometry and screen-space viewport transforms.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}
impl Point {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
    pub fn distance(self, other: Self) -> f64 {
        (self.x - other.x).hypot(self.y - other.y)
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}
impl Size {
    pub const fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub origin: Point,
    pub size: Size,
}
impl Rect {
    pub const fn new(origin: Point, size: Size) -> Self {
        Self { origin, size }
    }
    pub fn center(self) -> Point {
        Point::new(
            self.origin.x + self.size.width / 2.0,
            self.origin.y + self.size.height / 2.0,
        )
    }
    pub fn contains(self, point: Point) -> bool {
        point.x >= self.origin.x
            && point.y >= self.origin.y
            && point.x <= self.right()
            && point.y <= self.bottom()
    }
    pub fn contains_rect(self, other: Self) -> bool {
        self.contains(other.origin) && self.contains(Point::new(other.right(), other.bottom()))
    }
    pub fn right(self) -> f64 {
        self.origin.x + self.size.width
    }
    pub fn bottom(self) -> f64 {
        self.origin.y + self.size.height
    }
    pub fn intersects(self, other: Self) -> bool {
        self.origin.x <= other.right()
            && self.right() >= other.origin.x
            && self.origin.y <= other.bottom()
            && self.bottom() >= other.origin.y
    }

    /// Whether the rectangles share area. Touching boundaries are not overlaps.
    pub fn overlaps(self, other: Self) -> bool {
        self.right() > other.origin.x
            && other.right() > self.origin.x
            && self.bottom() > other.origin.y
            && other.bottom() > self.origin.y
    }
    pub fn union(self, other: Self) -> Self {
        let origin = Point::new(
            self.origin.x.min(other.origin.x),
            self.origin.y.min(other.origin.y),
        );
        Self::new(
            origin,
            Size::new(
                self.right().max(other.right()) - origin.x,
                self.bottom().max(other.bottom()) - origin.y,
            ),
        )
    }
    pub fn inflate(self, padding: f64) -> Self {
        Self::new(
            Point::new(self.origin.x - padding, self.origin.y - padding),
            Size::new(
                (self.size.width + 2.0 * padding).max(0.0),
                (self.size.height + 2.0 * padding).max(0.0),
            ),
        )
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Viewport {
    pub x: f64,
    pub y: f64,
    pub zoom: f64,
}
impl Default for Viewport {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ViewportLimits {
    pub min_zoom: f64,
    pub max_zoom: f64,
}
impl Default for ViewportLimits {
    fn default() -> Self {
        Self {
            min_zoom: 0.1,
            max_zoom: 2.0,
        }
    }
}
impl Viewport {
    pub fn world_to_screen(self, world: Point) -> Point {
        Point::new(world.x * self.zoom + self.x, world.y * self.zoom + self.y)
    }
    pub fn screen_to_world(self, screen: Point) -> Point {
        let zoom = valid_zoom(self.zoom);
        Point::new((screen.x - self.x) / zoom, (screen.y - self.y) / zoom)
    }
    pub fn zoom_at(self, screen: Point, factor: f64, min: f64, max: f64) -> Self {
        let world = self.screen_to_world(screen);
        let (min, max) = zoom_limits(min, max);
        let requested = valid_zoom(self.zoom) * factor;
        let zoom = if requested.is_nan() {
            valid_zoom(self.zoom).clamp(min, max)
        } else {
            requested.clamp(min, max)
        };
        Self {
            x: screen.x - world.x * zoom,
            y: screen.y - world.y * zoom,
            zoom,
        }
    }
    pub fn fit(bounds: Rect, container: Size, padding: f64, min_zoom: f64, max_zoom: f64) -> Self {
        let (min, max) = zoom_limits(min_zoom, max_zoom);
        let available = Size::new(
            (container.width - 2.0 * padding).max(0.0),
            (container.height - 2.0 * padding).max(0.0),
        );
        let sx = if bounds.size.width > 0.0 {
            available.width / bounds.size.width
        } else {
            max
        };
        let sy = if bounds.size.height > 0.0 {
            available.height / bounds.size.height
        } else {
            max
        };
        let zoom = sx.min(sy).clamp(min, max);
        let center = bounds.center();
        Self {
            x: container.width / 2.0 - center.x * zoom,
            y: container.height / 2.0 - center.y * zoom,
            zoom,
        }
    }
    pub fn visible_world_rect(self, container: Size) -> Rect {
        Rect::new(
            self.screen_to_world(Point::default()),
            Size::new(
                container.width / valid_zoom(self.zoom),
                container.height / valid_zoom(self.zoom),
            ),
        )
    }
}
fn valid_zoom(value: f64) -> f64 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        1.0
    }
}
fn zoom_limits(min: f64, max: f64) -> (f64, f64) {
    let min = valid_zoom(min);
    (min, valid_zoom(max).max(min))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn points_and_rectangles() {
        let rect = Rect::new(Point::new(10.0, 20.0), Size::new(30.0, 40.0));
        assert_eq!(rect.center(), Point::new(25.0, 40.0));
        assert!(rect.contains(Point::new(40.0, 60.0)));
        assert!(!rect.contains(Point::new(41.0, 60.0)));
        assert!(rect.intersects(Rect::new(Point::new(40.0, 20.0), Size::new(1.0, 1.0))));
        assert!(!rect.overlaps(Rect::new(Point::new(40.0, 20.0), Size::new(1.0, 1.0))));
        assert!(rect.overlaps(Rect::new(Point::new(39.0, 20.0), Size::new(1.0, 1.0))));
        assert!(!rect.intersects(Rect::new(Point::new(41.0, 20.0), Size::new(1.0, 1.0))));
        assert_eq!(
            rect.inflate(5.0),
            Rect::new(Point::new(5.0, 15.0), Size::new(40.0, 50.0))
        );
        assert!(rect.inflate(5.0).contains_rect(rect));
        assert_eq!(
            rect.union(Rect::new(Point::default(), Size::new(5.0, 5.0))),
            Rect::new(Point::default(), Size::new(40.0, 60.0))
        );
        assert_eq!(Point::default().distance(Point::new(3.0, 4.0)), 5.0);
    }
    #[test]
    fn transforms_zoom_fit() {
        let v = Viewport {
            x: 50.0,
            y: -10.0,
            zoom: 0.5,
        };
        let p = Point::new(10.0, 30.0);
        assert_eq!(v.screen_to_world(v.world_to_screen(p)), p);
        let cursor = Point::new(80.0, 15.0);
        let zoomed = v.zoom_at(cursor, 2.0, 0.1, 2.0);
        assert_eq!(zoomed.screen_to_world(cursor), v.screen_to_world(cursor));
        assert_eq!(v.zoom_at(cursor, 100.0, 0.1, 2.0).zoom, 2.0);
        let bounds = Rect::new(Point::new(-10.0, 20.0), Size::new(100.0, 50.0));
        let fitted = Viewport::fit(bounds, Size::new(300.0, 200.0), 10.0, 0.1, 2.0);
        assert_eq!(fitted.zoom, 2.0);
        assert_eq!(
            fitted.world_to_screen(bounds.center()),
            Point::new(150.0, 100.0)
        );
        assert_eq!(
            v.visible_world_rect(Size::new(200.0, 100.0)),
            Rect::new(Point::new(-100.0, 20.0), Size::new(400.0, 200.0))
        );
        assert!(
            Viewport::fit(Rect::default(), Size::default(), 0.0, 0.1, 2.0)
                .zoom
                .is_finite()
        );
        assert!(v.zoom_at(cursor, f64::NAN, 0.1, 2.0).zoom.is_finite());
    }
}
