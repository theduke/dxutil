//! SVG edge anchoring and stable path geometry.
use crate::geometry::{Point, Rect};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgeSide {
    Top,
    Right,
    Bottom,
    Left,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgeAnchor {
    /// The border point on the line between both node centres.
    #[default]
    Floating,
    /// The midpoint of a fixed side.
    Side(EdgeSide),
    /// The midpoint of the left or right side, whichever faces the other node.
    /// Suits horizontal hierarchies such as mind maps.
    Horizontal,
    /// The midpoint of the top or bottom side, whichever faces the other node.
    /// Suits vertical hierarchies such as top-down trees.
    Vertical,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgePathStyle {
    Straight,
    #[default]
    Bezier,
    Step,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgeMarker {
    #[default]
    None,
    Arrow,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EdgeStyle {
    pub path: EdgePathStyle,
    pub source_anchor: EdgeAnchor,
    pub target_anchor: EdgeAnchor,
    pub from_end: EdgeMarker,
    pub to_end: EdgeMarker,
    pub dashed: bool,
    pub class: Option<String>,
    /// Sideways bow of a curved edge as a fraction of its length (positive
    /// bows to the right of the direction of travel on screen). Secondary edges such as
    /// cross-links use it to stay distinguishable from straight hierarchy
    /// edges between the same or nearby ideas.
    #[serde(default)]
    pub curvature: f64,
}
impl Default for EdgeStyle {
    fn default() -> Self {
        Self {
            path: EdgePathStyle::Bezier,
            source_anchor: EdgeAnchor::Floating,
            target_anchor: EdgeAnchor::Floating,
            from_end: EdgeMarker::None,
            to_end: EdgeMarker::Arrow,
            dashed: false,
            class: None,
            curvature: 0.0,
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct EdgeGeometry {
    pub path_d: String,
    pub label_pos: Point,
    pub start: Point,
    pub end: Point,
}
/// Intersect a center-to-point ray with the border of a rectangle.
pub fn floating_anchor(rect: Rect, toward: Point) -> Point {
    let center = rect.center();
    let dx = toward.x - center.x;
    let dy = toward.y - center.y;
    if (dx == 0.0 && dy == 0.0) || rect.size.width <= 0.0 || rect.size.height <= 0.0 {
        return center;
    }
    let tx = if dx == 0.0 {
        f64::INFINITY
    } else {
        rect.size.width / 2.0 / dx.abs()
    };
    let ty = if dy == 0.0 {
        f64::INFINITY
    } else {
        rect.size.height / 2.0 / dy.abs()
    };
    let t = tx.min(ty);
    Point::new(center.x + dx * t, center.y + dy * t)
}
/// The midpoint of `side` of `rect`, where a port of that side sits.
pub fn side_anchor(rect: Rect, side: EdgeSide) -> Point {
    let center = rect.center();
    match side {
        EdgeSide::Top => Point::new(center.x, rect.origin.y),
        EdgeSide::Right => Point::new(rect.right(), center.y),
        EdgeSide::Bottom => Point::new(center.x, rect.bottom()),
        EdgeSide::Left => Point::new(rect.origin.x, center.y),
    }
}
/// How much of the vertical gap a horizontal gap needs to win: side-by-side
/// ideas read best connected left to right, as in mind maps.
const HORIZONTAL_BIAS: f64 = 0.75;
/// The side of `rect` facing `toward`: left or right unless the vertical gap
/// between both rectangles is clearly larger than the horizontal one, so
/// connections never leave through the long way round.
fn floating_side(rect: Rect, toward: Rect) -> EdgeSide {
    let horizontal_gap = (toward.origin.x - rect.right()).max(rect.origin.x - toward.right());
    let vertical_gap = (toward.origin.y - rect.bottom()).max(rect.origin.y - toward.bottom());
    let center = rect.center();
    let target = toward.center();
    match (
        horizontal_gap >= vertical_gap * HORIZONTAL_BIAS,
        target.x >= center.x,
        target.y >= center.y,
    ) {
        (true, true, _) => EdgeSide::Right,
        (true, false, _) => EdgeSide::Left,
        (false, _, true) => EdgeSide::Bottom,
        (false, _, false) => EdgeSide::Top,
    }
}
/// Resolve an anchor to a side midpoint and the side it leaves from.
/// Overlapping floating rectangles connect their centres and have no side.
fn anchor(rect: Rect, toward: Rect, kind: EdgeAnchor) -> (Point, Option<EdgeSide>) {
    let target = toward.center();
    let center = rect.center();
    let side = match kind {
        EdgeAnchor::Floating => {
            if rect.intersects(toward) {
                return (center, None);
            }
            // Snap to the midpoint of the side facing the other node: border
            // points near corners make curves leave at awkward angles.
            floating_side(rect, toward)
        }
        EdgeAnchor::Side(side) => side,
        EdgeAnchor::Horizontal => {
            if target.x >= center.x {
                EdgeSide::Right
            } else {
                EdgeSide::Left
            }
        }
        EdgeAnchor::Vertical => {
            if target.y >= center.y {
                EdgeSide::Bottom
            } else {
                EdgeSide::Top
            }
        }
    };
    (side_anchor(rect, side), Some(side))
}
/// The outward direction of a side; centre anchors point at the other end.
fn normal(side: Option<EdgeSide>, from: Point, to: Point) -> Point {
    match side {
        Some(EdgeSide::Top) => Point::new(0.0, -1.0),
        Some(EdgeSide::Right) => Point::new(1.0, 0.0),
        Some(EdgeSide::Bottom) => Point::new(0.0, 1.0),
        Some(EdgeSide::Left) => Point::new(-1.0, 0.0),
        None => {
            let len = from.distance(to).max(1.0);
            Point::new((to.x - from.x) / len, (to.y - from.y) / len)
        }
    }
}
/// Round to tenths without trailing decimal zeros or negative zero.
fn number(value: f64) -> String {
    let value = (value * 10.0).round() / 10.0;
    if value == 0.0 {
        "0".into()
    } else {
        value.to_string()
    }
}
fn xy(p: Point) -> String {
    format!("{} {}", number(p.x), number(p.y))
}
pub fn edge_geometry(
    source: Rect,
    target: Rect,
    style: &EdgeStyle,
    self_loop: bool,
) -> EdgeGeometry {
    edge_geometry_with_offset(source, target, style, 0, self_loop)
}
/// `offset_index` separates parallel edges. Self-loops are determined by coincident rectangles.
pub fn edge_geometry_with_offset(
    source: Rect,
    target: Rect,
    style: &EdgeStyle,
    offset_index: i32,
    self_loop: bool,
) -> EdgeGeometry {
    if self_loop {
        let start = Point::new(source.right(), source.origin.y + source.size.height * 0.25);
        let end = Point::new(source.origin.x + source.size.width * 0.75, source.origin.y);
        let distance = 40.0 + f64::from(offset_index.unsigned_abs()) * 12.0;
        let a = Point::new(start.x + distance, start.y - distance);
        let b = Point::new(end.x + distance, end.y - distance);
        return EdgeGeometry {
            path_d: format!("M {} C {} {} {}", xy(start), xy(a), xy(b), xy(end)),
            label_pos: cubic_midpoint(start, a, b, end),
            start,
            end,
        };
    }
    let (start, start_side) = anchor(source, target, style.source_anchor);
    let (end, end_side) = anchor(target, source, style.target_anchor);
    let midpoint = Point::new((start.x + end.x) / 2.0, (start.y + end.y) / 2.0);
    let (path_d, label_pos) = match style.path {
        EdgePathStyle::Straight => (format!("M {} L {}", xy(start), xy(end)), midpoint),
        EdgePathStyle::Bezier => {
            let (a, b) = bezier_handles(
                (start, start_side),
                (end, end_side),
                f64::from(offset_index) * 12.0,
                style.curvature,
            );
            (
                format!("M {} C {} {} {}", xy(start), xy(a), xy(b), xy(end)),
                cubic_midpoint(start, a, b, end),
            )
        }
        EdgePathStyle::Step => {
            let horizontal = |side: Option<EdgeSide>| match side {
                Some(EdgeSide::Left | EdgeSide::Right) => true,
                Some(EdgeSide::Top | EdgeSide::Bottom) => false,
                None => (end.x - start.x).abs() >= (end.y - start.y).abs(),
            };
            let corners = match (horizontal(start_side), horizontal(end_side)) {
                // Perpendicular sides, such as a tree's leaf column hanging
                // from its parent's bottom: one elbow.
                (false, true) if end_side.is_some() => vec![Point::new(start.x, end.y)],
                (true, false) if end_side.is_some() => vec![Point::new(end.x, start.y)],
                // Parallel sides: an org chart bus halfway between both ends,
                // shared by siblings.
                (true, _) => vec![
                    Point::new(midpoint.x, start.y),
                    Point::new(midpoint.x, end.y),
                ],
                (false, _) => vec![
                    Point::new(start.x, midpoint.y),
                    Point::new(end.x, midpoint.y),
                ],
            };
            let mut points = Vec::with_capacity(corners.len() + 2);
            points.push(start);
            points.extend(corners);
            points.push(end);
            let label_pos = if points.len() == 3 {
                points[1]
            } else {
                midpoint
            };
            (rounded_step(&points), label_pos)
        }
    };
    EdgeGeometry {
        path_d,
        label_pos,
        start,
        end,
    }
}
/// Bezier control points for a curve leaving `start` and entering `end` along
/// their side normals. `lane` (pixels) separates parallel edges and
/// `curvature` (a fraction of the length) bows the curve sideways.
fn bezier_handles(
    (start, start_side): (Point, Option<EdgeSide>),
    (end, end_side): (Point, Option<EdgeSide>),
    lane: f64,
    curvature: f64,
) -> (Point, Point) {
    let distance = start.distance(end);
    let n1 = normal(start_side, start, end);
    let n2 = normal(end_side, end, start);
    // Handles scale with the span along the side normals so short hops stay
    // tight and long ones sweep.
    let signed_along = (end.x - start.x) * n1.x + (end.y - start.y) * n1.y;
    let along = signed_along.abs();
    let mut offset = (along.max(distance * 0.25) * 0.5).clamp(20.0, 160.0);
    // Facing sides (a tree's parent bottom and child top): handles longer
    // than half the gap overshoot the other row, so wide sibling fans dip
    // past their targets and braid. Half the gap gives an even S-curve.
    let facing = signed_along > 0.0 && (n1.x + n2.x).abs() + (n1.y + n2.y).abs() < 1e-9;
    if facing {
        offset = offset.min(along * 0.5);
    }
    let bend = lane + curvature * distance;
    let denominator = distance.max(1.0);
    let perpendicular = Point::new(
        -(end.y - start.y) / denominator * bend,
        (end.x - start.x) / denominator * bend,
    );
    (
        Point::new(
            start.x + n1.x * offset + perpendicular.x,
            start.y + n1.y * offset + perpendicular.y,
        ),
        Point::new(
            end.x + n2.x * offset + perpendicular.x,
            end.y + n2.y * offset + perpendicular.y,
        ),
    )
}
/// Corner radius of elbow connectors.
const STEP_RADIUS: f64 = 10.0;
/// An orthogonal polyline through `points` with rounded corners. Radii
/// shrink to fit short segments; straight runs stay plain lines.
fn rounded_step(points: &[Point]) -> String {
    let radius = points
        .windows(2)
        .map(|pair| pair[0].distance(pair[1]) / 2.0)
        .fold(STEP_RADIUS, f64::min);
    let (Some(&first), Some(&last)) = (points.first(), points.last()) else {
        return String::new();
    };
    let mut path = format!("M {}", xy(first));
    if radius < 0.5 {
        for point in &points[1..] {
            path.push_str(&format!(" L {}", xy(*point)));
        }
        return path;
    }
    let toward = |from: Point, to: Point, by: f64| {
        let length = from.distance(to).max(f64::EPSILON);
        Point::new(
            from.x + (to.x - from.x) / length * by,
            from.y + (to.y - from.y) / length * by,
        )
    };
    for corner in points.windows(3) {
        let (before, at, after) = (corner[0], corner[1], corner[2]);
        path.push_str(&format!(
            " L {} Q {} {}",
            xy(toward(at, before, radius)),
            xy(at),
            xy(toward(at, after, radius))
        ));
    }
    path.push_str(&format!(" L {}", xy(last)));
    path
}
/// Clearance kept between a routed edge and other nodes.
const OBSTACLE_PADDING: f64 = 6.0;
/// How much longer than the centre distance a re-routed edge may get.
const MAX_DETOUR: f64 = 1.8;
/// Route cost, as a fraction of the centre distance, of an end leaving
/// straight away from the other node (scaled down for partly facing sides).
const AWAY_PENALTY: f64 = 0.6;
/// Route cost, as a fraction of the centre distance, of an end attaching to a
/// side that already carries another connection (such as the hierarchy edge
/// into a mind map idea), where both would merge.
const BUSY_PENALTY: f64 = 0.35;
/// A curved secondary edge re-routed around other nodes.
#[derive(Clone, Debug, PartialEq)]
pub struct EdgeRoute {
    pub style: EdgeStyle,
    /// The route avoids every obstacle. Otherwise it is the least obstructed
    /// one, and renderers should draw the edge above the nodes it crosses.
    pub clear: bool,
}
/// The sides an edge with `style` leaves `source` from and enters `target`
/// at; `None` for centre anchors of overlapping nodes.
pub fn edge_sides(
    source: Rect,
    target: Rect,
    style: &EdgeStyle,
) -> (Option<EdgeSide>, Option<EdgeSide>) {
    (
        anchor(source, target, style.source_anchor).1,
        anchor(target, source, style.target_anchor).1,
    )
}
/// Sides of a secondary edge's end nodes that already carry other
/// connections; routes avoid attaching there.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BusySides<'a> {
    pub source: &'a [EdgeSide],
    pub target: &'a [EdgeSide],
}
/// Re-route a curved secondary edge such as a cross-link around `obstacles`,
/// the other nodes. Candidates are the configured bow, the mirrored bow and
/// stronger ones, each with the configured anchors, with both ends on the
/// same side (an arc around whatever lies between them) and with any pair of
/// sides. An end anchored to a fixed [`EdgeAnchor::Side`] keeps that side in
/// every candidate. Routes with long detours or through obstacles are
/// rejected; the rest are scored by length plus penalties for ends facing
/// away from the other node or attaching to `busy` sides, and the cheapest
/// wins. Returns
/// `None` when the configured route is clear and unpenalised or the edge does
/// not bow (hierarchy edges keep their layout-defined shape); a blocked edge
/// keeps its route.
pub fn route_around(
    source: Rect,
    target: Rect,
    style: &EdgeStyle,
    offset_index: i32,
    obstacles: &[Rect],
    busy: BusySides<'_>,
) -> Option<EdgeRoute> {
    let base = style.curvature;
    if base == 0.0 || style.path != EdgePathStyle::Bezier {
        return None;
    }
    let lane = f64::from(offset_index) * 12.0;
    let distance = source.center().distance(target.center()).max(1.0);
    // A clear route may be at most this much longer than the straight line
    // between the node centres; longer detours read as unrelated loops, so the
    // edge is drawn over the nodes instead.
    let max_length = distance * MAX_DETOUR;
    let obstacles = obstacles
        .iter()
        .map(|rect| rect.inflate(OBSTACLE_PADDING))
        .collect::<Vec<_>>();
    let penalty = |(point, side): (Point, Option<EdgeSide>), toward: Rect, busy: &[EdgeSide]| {
        let Some(side) = side else {
            return 0.0;
        };
        let outward = normal(Some(side), point, point);
        let center = toward.center();
        let length = point.distance(center).max(1.0);
        let facing = ((center.x - point.x) * outward.x + (center.y - point.y) * outward.y) / length;
        let away = AWAY_PENALTY * (-facing).max(0.0);
        let merged = if busy.contains(&side) {
            BUSY_PENALTY
        } else {
            0.0
        };
        (away + merged) * distance
    };
    // The penalty and the sampled curve of the route through `anchors` with
    // `curvature`, or `None` when it is too long.
    let candidate = |anchors: (EdgeAnchor, EdgeAnchor), curvature: f64| {
        let start = anchor(source, target, anchors.0);
        let end = anchor(target, source, anchors.1);
        let (a, b) = bezier_handles(start, end, lane, curvature);
        let samples = 32;
        let points = (0..=samples)
            .map(|step| cubic_point(start.0, a, b, end.0, f64::from(step) / f64::from(samples)))
            .collect::<Vec<_>>();
        let length = points
            .windows(2)
            .map(|pair| pair[0].distance(pair[1]))
            .sum::<f64>();
        (length <= max_length).then(|| {
            let penalty = penalty(start, target, busy.source) + penalty(end, source, busy.target);
            (length, penalty, points)
        })
    };
    // Routes must also leave their own end nodes outwards instead of
    // doubling back through them.
    let ends = [source.inflate(-1.0), target.inflate(-1.0)];
    let clear = |points: &[Point]| {
        let inner = &points[1..points.len() - 1];
        obstacles
            .iter()
            .chain(&ends)
            .all(|rect| !inner.iter().any(|point| rect.contains(*point)))
    };
    let configured = (style.source_anchor, style.target_anchor);
    if let Some((_, penalty, points)) = candidate(configured, base)
        && penalty == 0.0
        && clear(&points)
    {
        return None;
    }
    let sides = [
        EdgeSide::Right,
        EdgeSide::Left,
        EdgeSide::Top,
        EdgeSide::Bottom,
    ];
    let mut anchors = vec![configured];
    let same = sides.map(|side| (EdgeAnchor::Side(side), EdgeAnchor::Side(side)));
    let mixed = sides.iter().flat_map(|from| {
        sides
            .iter()
            .filter(move |to| *to != from)
            .map(move |to| (EdgeAnchor::Side(*from), EdgeAnchor::Side(*to)))
    });
    // Fixed sides were chosen by the user: only the other end may move.
    let keeps = |fixed: EdgeAnchor, anchor: EdgeAnchor| {
        !matches!(fixed, EdgeAnchor::Side(_)) || anchor == fixed
    };
    for pair in same.into_iter().chain(mixed) {
        if keeps(configured.0, pair.0) && keeps(configured.1, pair.1) && !anchors.contains(&pair) {
            anchors.push(pair);
        }
    }
    let mut best: Option<(f64, (EdgeAnchor, EdgeAnchor), f64)> = None;
    for curvature in [
        base,
        -base,
        2.0 * base,
        -2.0 * base,
        3.0 * base,
        -3.0 * base,
    ] {
        for &pair in &anchors {
            let Some((length, penalty, points)) = candidate(pair, curvature) else {
                continue;
            };
            let cost = length + penalty;
            // Strictly cheaper only: earlier candidates win ties.
            if best.is_none_or(|(best, _, _)| cost < best) && clear(&points) {
                best = Some((cost, pair, curvature));
            }
        }
    }
    Some(match best {
        Some((_, pair, curvature)) if pair == configured && curvature == base => return None,
        Some((_, (source_anchor, target_anchor), curvature)) => EdgeRoute {
            style: EdgeStyle {
                source_anchor,
                target_anchor,
                curvature,
                ..style.clone()
            },
            clear: true,
        },
        None => EdgeRoute {
            style: style.clone(),
            clear: false,
        },
    })
}
fn cubic_point(p: Point, a: Point, b: Point, q: Point, t: f64) -> Point {
    let u = 1.0 - t;
    let (w0, w1, w2, w3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    Point::new(
        w0 * p.x + w1 * a.x + w2 * b.x + w3 * q.x,
        w0 * p.y + w1 * a.y + w2 * b.y + w3 * q.y,
    )
}
fn cubic_midpoint(p: Point, a: Point, b: Point, q: Point) -> Point {
    Point::new(
        (p.x + 3.0 * a.x + 3.0 * b.x + q.x) / 8.0,
        (p.y + 3.0 * a.y + 3.0 * b.y + q.y) / 8.0,
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::Size;
    #[test]
    fn quadrants_and_sides() {
        let r = Rect::new(Point::default(), Size::new(100.0, 100.0));
        for (to, expected) in [
            (Point::new(200.0, 200.0), Point::new(100.0, 100.0)),
            (Point::new(-100.0, 200.0), Point::new(0.0, 100.0)),
            (Point::new(-100.0, -100.0), Point::new(0.0, 0.0)),
            (Point::new(200.0, -100.0), Point::new(100.0, 0.0)),
        ] {
            assert_eq!(floating_anchor(r, to), expected);
        }
        for (side, expected) in [
            (EdgeSide::Top, Point::new(50.0, 0.0)),
            (EdgeSide::Right, Point::new(100.0, 50.0)),
            (EdgeSide::Bottom, Point::new(50.0, 100.0)),
            (EdgeSide::Left, Point::new(0.0, 50.0)),
        ] {
            assert_eq!(side_anchor(r, side), expected);
        }
    }
    #[test]
    fn exact_paths_degenerate_and_parallel() {
        let a = Rect::new(Point::default(), Size::new(100.0, 100.0));
        let b = Rect::new(Point::new(200.0, 0.0), a.size);
        let mut style = EdgeStyle {
            path: EdgePathStyle::Straight,
            ..Default::default()
        };
        assert_eq!(
            edge_geometry(a, b, &style, false).path_d,
            "M 100 50 L 200 50"
        );
        style.path = EdgePathStyle::Step;
        assert_eq!(
            edge_geometry(a, b, &style, false).path_d,
            "M 100 50 L 150 50 L 150 50 L 200 50"
        );
        let below = Rect::new(Point::new(300.0, 200.0), a.size);
        assert_eq!(
            edge_geometry(a, below, &style, false).path_d,
            "M 100 50 L 190 50 Q 200 50 200 60 L 200 240 Q 200 250 210 250 L 300 250"
        );
        style.path = EdgePathStyle::Bezier;
        assert_eq!(
            edge_geometry(a, b, &style, false).path_d,
            "M 100 50 C 150 50 150 50 200 50"
        );
        assert_ne!(
            edge_geometry_with_offset(a, b, &style, 1, false).path_d,
            edge_geometry(a, b, &style, false).path_d
        );
        assert_eq!(
            edge_geometry(a, a, &style, true).path_d,
            "M 100 25 C 140 -15 115 -40 75 0"
        );
        let overlap = Rect::new(Point::new(10.0, 10.0), a.size);
        assert_eq!(edge_geometry(a, overlap, &style, false).start, a.center());
        let coincident = edge_geometry(a, a, &style, false);
        assert_eq!(coincident.start, a.center());
        assert_eq!(coincident.end, a.center());
        assert_ne!(coincident.path_d, edge_geometry(a, a, &style, true).path_d);
        assert_eq!(number(-0.01), "0");
        assert_eq!(number(1.26), "1.3");
    }

    #[test]
    fn curved_diagonal_edges_leave_and_enter_along_side_normals() {
        let a = Rect::new(Point::default(), Size::new(100.0, 60.0));
        let b = Rect::new(Point::new(300.0, 200.0), a.size);
        // Floating anchors stay on the border, but the curve must not collapse
        // into the straight centre line: handles follow the crossed sides.
        let wide = Rect::new(Point::new(400.0, 100.0), a.size);
        let geometry = edge_geometry(a, wide, &EdgeStyle::default(), false);
        // Floating anchors snap to the facing sides' midpoints.
        assert_eq!(geometry.start, Point::new(100.0, 30.0));
        assert_eq!(geometry.path_d, "M 100 30 C 250 30 250 130 400 130");
        let tall = edge_geometry(a, b, &EdgeStyle::default(), false);
        // Wider apart than above each other: left to right.
        assert_eq!(tall.path_d, "M 100 30 C 200 30 200 230 300 230");
        let below = Rect::new(Point::new(150.0, 200.0), a.size);
        assert_eq!(
            edge_geometry(a, below, &EdgeStyle::default(), false).path_d,
            "M 50 60 C 50 130 200 130 200 200"
        );
        for (anchor, start, end) in [
            (
                EdgeAnchor::Horizontal,
                Point::new(100.0, 30.0),
                Point::new(300.0, 230.0),
            ),
            (
                EdgeAnchor::Vertical,
                Point::new(50.0, 60.0),
                Point::new(350.0, 200.0),
            ),
        ] {
            let style = EdgeStyle {
                source_anchor: anchor,
                target_anchor: anchor,
                ..Default::default()
            };
            let forward = edge_geometry(a, b, &style, false);
            assert_eq!((forward.start, forward.end), (start, end));
            let backward = edge_geometry(b, a, &style, false);
            assert_eq!((backward.start, backward.end), (end, start));
        }
    }

    #[test]
    fn opposite_direction_lanes_are_separate_and_centered() {
        let a = Rect::new(Point::default(), Size::new(100.0, 100.0));
        let b = Rect::new(Point::new(200.0, 0.0), a.size);
        let forward = edge_geometry_with_offset(a, b, &EdgeStyle::default(), -1, false);
        let reverse = edge_geometry_with_offset(b, a, &EdgeStyle::default(), -1, false);
        assert!(forward.label_pos.y < 50.0);
        assert!(reverse.label_pos.y > 50.0);
        assert_eq!(forward.label_pos.y + reverse.label_pos.y, 100.0);
    }

    #[test]
    fn curvature_bows_an_edge_away_from_the_straight_line() {
        let a = Rect::new(Point::default(), Size::new(100.0, 40.0));
        let b = Rect::new(Point::new(400.0, 0.0), a.size);
        let straight = edge_geometry(a, b, &EdgeStyle::default(), false);
        let bowed = edge_geometry(
            a,
            b,
            &EdgeStyle {
                curvature: 0.25,
                ..Default::default()
            },
            false,
        );
        assert_eq!((straight.start, straight.end), (bowed.start, bowed.end));
        // A quarter of the 300px span between the facing sides, at 3/4 depth.
        assert!((bowed.label_pos.y - straight.label_pos.y - 56.25).abs() < 0.01);
    }

    /// Every cubic control point of `path` (`M p C a b q`).
    fn controls(path: &str) -> Vec<Point> {
        let numbers = path
            .split(|c: char| c.is_ascii_alphabetic() || c.is_whitespace())
            .filter(|part| !part.is_empty())
            .map(|part| part.parse::<f64>().unwrap())
            .collect::<Vec<_>>();
        numbers
            .chunks(2)
            .map(|pair| Point::new(pair[0], pair[1]))
            .collect()
    }

    #[test]
    fn wide_tree_fans_never_overshoot_the_child_row() {
        let style = EdgeStyle {
            source_anchor: EdgeAnchor::Vertical,
            target_anchor: EdgeAnchor::Vertical,
            ..Default::default()
        };
        let parent = Rect::new(Point::new(0.0, 0.0), Size::new(120.0, 40.0));
        let mut previous = None;
        for x in [-1400.0, -900.0, -400.0, 0.0, 400.0, 900.0, 1400.0] {
            let child = Rect::new(Point::new(x, 110.0), parent.size);
            let geometry = edge_geometry(parent, child, &style, false);
            let points = controls(&geometry.path_d);
            // Handles stay between the rows, so the curve is monotonic in y.
            for point in &points {
                assert!(
                    (40.0..=110.0).contains(&point.y),
                    "{x}: {}",
                    geometry.path_d
                );
            }
            // Siblings fan out without crossing: the curve's midpoint moves
            // monotonically with the child.
            if let Some(previous) = previous {
                assert!(geometry.label_pos.x > previous);
            }
            previous = Some(geometry.label_pos.x);
        }
        // Horizontal hierarchies (mind maps, Tree →) behave the same way.
        let style = EdgeStyle {
            source_anchor: EdgeAnchor::Horizontal,
            target_anchor: EdgeAnchor::Horizontal,
            ..Default::default()
        };
        let child = Rect::new(Point::new(200.0, 900.0), parent.size);
        for point in controls(&edge_geometry(parent, child, &style, false).path_d) {
            assert!((120.0..=200.0).contains(&point.x));
        }
    }

    #[test]
    fn diagonal_neighbours_connect_side_to_side_unless_stacked() {
        let a = Rect::new(Point::default(), Size::new(120.0, 50.0));
        // A mind map child up and to the right: left to right.
        let child = Rect::new(Point::new(210.0, -150.0), a.size);
        assert_eq!(floating_side(a, child), EdgeSide::Right);
        assert_eq!(floating_side(child, a), EdgeSide::Left);
        // A wide neighbour just beside but well below: top to bottom.
        let below = Rect::new(Point::new(140.0, 120.0), Size::new(240.0, 50.0));
        assert_eq!(floating_side(a, below), EdgeSide::Bottom);
        assert_eq!(floating_side(below, a), EdgeSide::Top);
    }

    #[test]
    fn floating_anchors_use_side_midpoints() {
        let a = Rect::new(Point::default(), Size::new(100.0, 40.0));
        for (other, start) in [
            (Point::new(260.0, 90.0), Point::new(100.0, 20.0)),
            (Point::new(-260.0, -90.0), Point::new(0.0, 20.0)),
            (Point::new(30.0, 200.0), Point::new(50.0, 40.0)),
            (Point::new(-30.0, -200.0), Point::new(50.0, 0.0)),
        ] {
            let b = Rect::new(other, a.size);
            assert_eq!(
                edge_geometry(a, b, &EdgeStyle::default(), false).start,
                start
            );
        }
    }

    #[test]
    fn bowed_edges_route_around_nodes_in_between() {
        let a = Rect::new(Point::default(), Size::new(100.0, 40.0));
        let b = Rect::new(Point::new(500.0, 0.0), a.size);
        let style = EdgeStyle {
            curvature: 0.25,
            ..Default::default()
        };
        // Nothing in the way: keep the configured route.
        assert_eq!(
            route_around(a, b, &style, 0, &[], BusySides::default()),
            None
        );
        let clear = Rect::new(Point::new(250.0, 300.0), a.size);
        assert_eq!(
            route_around(a, b, &style, 0, &[clear], BusySides::default()),
            None
        );
        // A tall note between both ends: the bow flips or grows until the
        // curve clears it.
        let note = Rect::new(Point::new(250.0, -20.0), Size::new(100.0, 150.0));
        let routed = route_around(a, b, &style, 0, &[note], BusySides::default()).unwrap();
        assert!(routed.clear);
        let geometry = edge_geometry(a, b, &routed.style, false);
        assert!(!note.inflate(OBSTACLE_PADDING).contains(geometry.label_pos));
        // Straight hierarchy edges are never re-routed.
        assert_eq!(
            route_around(
                a,
                b,
                &EdgeStyle::default(),
                0,
                &[note],
                BusySides::default()
            ),
            None
        );
    }

    #[test]
    fn cross_links_arc_around_a_wide_node_between_stacked_ends() {
        // Personas above and right of a wide note, Interviews below its left
        // end (a mind map column).
        let personas = Rect::new(Point::new(400.0, 0.0), Size::new(120.0, 50.0));
        let note = Rect::new(Point::new(150.0, 80.0), Size::new(400.0, 260.0));
        let interviews = Rect::new(Point::new(150.0, 370.0), Size::new(140.0, 50.0));
        let style = EdgeStyle {
            curvature: 0.25,
            ..Default::default()
        };
        let routed = route_around(
            personas,
            interviews,
            &style,
            0,
            &[note],
            BusySides::default(),
        )
        .unwrap();
        assert!(routed.clear);
        let geometry = edge_geometry(personas, interviews, &routed.style, false);
        let points = controls(&geometry.path_d);
        let (start, a, b, end) = (points[0], points[1], points[2], points[3]);
        for step in 1..32 {
            let point = cubic_point(start, a, b, end, f64::from(step) / 32.0);
            assert!(
                !note.contains(point),
                "{} crosses the note",
                geometry.path_d
            );
        }
        // With a neighbour boxing the detour in, the edge keeps its route
        // and is flagged for drawing above the nodes.
        let wall = Rect::new(Point::new(560.0, -200.0), Size::new(400.0, 800.0));
        let left = Rect::new(Point::new(-300.0, -200.0), Size::new(440.0, 800.0));
        let blocked = route_around(
            personas,
            interviews,
            &style,
            0,
            &[note, wall, left],
            BusySides::default(),
        )
        .unwrap();
        assert!(!blocked.clear);
        assert_eq!(blocked.style, style);
    }

    #[test]
    fn org_chart_connectors_share_a_bus_or_hang_off_a_spine() {
        let parent = Rect::new(Point::new(0.0, 0.0), Size::new(120.0, 40.0));
        let style = EdgeStyle {
            path: EdgePathStyle::Step,
            source_anchor: EdgeAnchor::Vertical,
            target_anchor: EdgeAnchor::Vertical,
            ..Default::default()
        };
        // Siblings in the next row share the horizontal bus halfway down.
        for (x, path) in [
            (
                -300.0,
                "M 60 40 L 60 70 Q 60 80 50 80 L -230 80 Q -240 80 -240 90 L -240 120",
            ),
            (
                300.0,
                "M 60 40 L 60 70 Q 60 80 70 80 L 350 80 Q 360 80 360 90 L 360 120",
            ),
        ] {
            let child = Rect::new(Point::new(x, 120.0), parent.size);
            assert_eq!(edge_geometry(parent, child, &style, false).path_d, path);
        }
        // A stacked leaf hangs off the spine below the parent's centre and
        // enters its left side through one rounded elbow.
        let leaf = Rect::new(Point::new(84.0, 60.0), Size::new(140.0, 40.0));
        let hanging = EdgeStyle {
            source_anchor: EdgeAnchor::Side(EdgeSide::Bottom),
            target_anchor: EdgeAnchor::Side(EdgeSide::Left),
            ..style
        };
        assert_eq!(
            edge_geometry(parent, leaf, &hanging, false).path_d,
            "M 60 40 L 60 70 Q 60 80 70 80 L 84 80"
        );
    }

    #[test]
    fn cross_links_prefer_facing_free_sides() {
        // A mind map: Social hangs left of the root, Research right of it.
        // Their hierarchy edges enter Social's right and Research's left side.
        let root = Rect::new(Point::new(-90.0, -25.0), Size::new(180.0, 50.0));
        let social = Rect::new(Point::new(-420.0, 60.0), Size::new(120.0, 40.0));
        let research = Rect::new(Point::new(300.0, 40.0), Size::new(120.0, 40.0));
        let style = EdgeStyle {
            curvature: 0.25,
            ..Default::default()
        };
        let busy = BusySides {
            source: &[EdgeSide::Right],
            target: &[EdgeSide::Left],
        };
        let routed = route_around(social, research, &style, 0, &[root], busy).unwrap();
        assert!(routed.clear);
        let (from, to) = edge_sides(social, research, &routed.style);
        // Never out of the far side, and never into the hierarchy edge's side.
        assert_ne!(from, Some(EdgeSide::Left));
        assert_ne!(to, Some(EdgeSide::Left));
        assert_ne!(to, Some(EdgeSide::Right));
        let geometry = edge_geometry(social, research, &routed.style, false);
        let points = controls(&geometry.path_d);
        for step in 1..32 {
            let point = cubic_point(
                points[0],
                points[1],
                points[2],
                points[3],
                f64::from(step) / 32.0,
            );
            for end in [social, research] {
                assert!(!end.inflate(-1.0).contains(point), "{}", geometry.path_d);
            }
        }
        for step in 1..32 {
            let point = cubic_point(
                points[0],
                points[1],
                points[2],
                points[3],
                f64::from(step) / 32.0,
            );
            assert!(!root.contains(point), "{}", geometry.path_d);
        }
    }

    #[test]
    fn rerouting_keeps_fixed_sides() {
        // The same blocked cross-link as above, but drawn from Social's
        // bottom port into Research's top port.
        let root = Rect::new(Point::new(-90.0, -25.0), Size::new(180.0, 50.0));
        let social = Rect::new(Point::new(-420.0, 60.0), Size::new(120.0, 40.0));
        let research = Rect::new(Point::new(300.0, 40.0), Size::new(120.0, 40.0));
        let busy = BusySides {
            source: &[EdgeSide::Bottom],
            target: &[EdgeSide::Top],
        };
        for (source, target) in [
            (EdgeSide::Bottom, EdgeAnchor::Side(EdgeSide::Top)),
            (EdgeSide::Bottom, EdgeAnchor::Floating),
        ] {
            let style = EdgeStyle {
                curvature: 0.25,
                source_anchor: EdgeAnchor::Side(source),
                target_anchor: target,
                ..Default::default()
            };
            let routed = route_around(social, research, &style, 0, &[root], busy)
                .map_or(style.clone(), |route| route.style);
            assert_eq!(routed.source_anchor, EdgeAnchor::Side(source));
            if matches!(target, EdgeAnchor::Side(_)) {
                assert_eq!(routed.target_anchor, target);
            }
            let (from, to) = edge_sides(social, research, &routed);
            assert_eq!(from, Some(EdgeSide::Bottom));
            if matches!(target, EdgeAnchor::Side(_)) {
                assert_eq!(to, Some(EdgeSide::Top));
            }
        }
    }
}
