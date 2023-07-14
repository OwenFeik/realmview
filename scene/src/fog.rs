use crate::{comms::SceneEvent, Point};

/// A `struct Fog` represents fog of war over the scene. It keeps a bit array
/// indicating whether each tile is occluded as it's representation of the fog.
///
/// Representation is like so:
/// - For each row in the grid, we have the smallest number of u32s
///   necessary to represent the tiles of the row.
/// - This is ceil(w / 32) u32s per row and h rows.
/// - Then to check if (x, y) is occluded, we find the relevant u32 at
///   index ceil(w / 32) * y + (x / 32)
///
/// A 1 implies that that cell is clear while a 0 implies that the cell is
/// occluded.

#[derive(Clone, serde_derive::Deserialize, serde_derive::Serialize)]
pub struct Fog {
    pub active: bool,
    pub w: u32,
    pub h: u32,

    /// Number of revealed cells
    pub n_revealed: u32,

    fog: Vec<u32>,
}

impl Fog {
    const BITS: u32 = 32;

    pub fn new(w: u32, h: u32) -> Fog {
        Fog {
            active: false,
            w,
            h,
            n_revealed: 0,
            fog: Self::make_fog(w, h),
        }
    }

    pub fn bytes(&self) -> Vec<u8> {
        self.fog.iter().flat_map(|f| f.to_be_bytes()).collect()
    }

    pub fn from_bytes(w: u32, h: u32, bytes: &[u8]) -> Self {
        let fog: Vec<u32> = bytes
            .chunks_exact(32 / 8)
            .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
            .collect();

        let mut n_revealed = 0;
        for line in fog.iter() {
            n_revealed += line.count_ones();
        }

        Self {
            active: false,
            w,
            h,
            n_revealed,
            fog,
        }
    }

    fn row_ints(w: u32) -> u32 {
        w.div_ceil(Self::BITS)
    }

    fn make_fog(w: u32, h: u32) -> Vec<u32> {
        vec![0; (Self::row_ints(w) * h) as usize]
    }

    fn row_len(&self) -> u32 {
        Self::row_ints(self.w)
    }

    fn nth_row(&self, y: u32) -> u32 {
        self.row_len() * y
    }

    fn idx(&self, x: u32, y: u32) -> usize {
        (self.nth_row(y) + x / Self::BITS) as usize
    }

    fn on_map(&self, x: u32, y: u32) -> bool {
        x < self.w && y < self.h
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        let mut new_fog = Self::make_fog(w, h);

        for y in 0..h {
            for x in 0..w {
                if self.on_map(x, y) {
                    let idx = (Self::row_ints(w) * y + (x / Self::BITS)) as usize;
                    new_fog[idx] = self.fog[self.idx(x, y)];
                }
            }
        }

        self.w = w;
        self.h = h;
        self.fog = new_fog;
    }

    pub fn occluded(&self, x: u32, y: u32) -> bool {
        if !self.on_map(x, y) {
            return true;
        }

        if let Some(int) = self.fog.get(self.idx(x, y)) {
            *int & 1 << (x % Self::BITS) == 0
        } else {
            false
        }
    }

    pub fn rect_occluded(&self, rect: crate::Rect) -> bool {
        if !self.active {
            return false;
        }

        let x0 = rect.x.floor().max(0.0) as u32;
        let y0 = rect.y.floor().max(0.0) as u32;
        let x1 = (rect.x + rect.w).max(0.0) as u32;
        let y1 = (rect.y + rect.h).max(0.0) as u32;

        for x in x0..x1 {
            for y in y0..y1 {
                if !self.occluded(x, y) {
                    return false;
                }
            }
        }
        true
    }

    pub fn reveal(&mut self, x: u32, y: u32) -> Option<SceneEvent> {
        if !self.on_map(x, y) {
            return None;
        }

        if !self.occluded(x, y) {
            return None;
        }

        self.n_revealed += 1;
        let index = self.idx(x, y);
        if let Some(row) = self.fog.get_mut(index) {
            *row |= 1 << (x % Self::BITS);
        }

        Some(SceneEvent::FogReveal(true, x, y))
    }

    pub fn occlude(&mut self, x: u32, y: u32) -> Option<SceneEvent> {
        if !self.on_map(x, y) {
            return None;
        }

        if self.occluded(x, y) {
            return None;
        }

        self.n_revealed -= 1;
        let index = self.idx(x, y);
        if let Some(row) = self.fog.get_mut(index) {
            *row &= !(1 << (x % Self::BITS));
        }

        Some(SceneEvent::FogOcclude(false, x, y))
    }

    pub fn set(&mut self, x: u32, y: u32, occluded: bool) -> Option<SceneEvent> {
        if occluded {
            self.occlude(x, y)
        } else {
            self.reveal(x, y)
        }
    }

    fn tile_center(x: u32, y: u32) -> Point {
        Point::new(x as f32 + 0.5, y as f32 + 0.5)
    }

    /// Set occluded status of all tiles whose center is within a given radius
    /// of a point.
    ///
    /// * `at`       Point around which to update tile state.
    /// * `r`        Radius around `at` to update tile state.
    /// * `occluded` New occluded state for tiles in range.
    pub fn set_circle(&mut self, at: Point, r: f32, occluded: bool) -> SceneEvent {
        let mut events = Vec::new();

        // Negative values become 0 through (as u32).
        let xmin = (at.x - r).floor() as u32;
        let xmax = (at.x + r).ceil() as u32;
        let ymin = (at.y - r).floor() as u32;
        let ymax = (at.y + r).ceil() as u32;

        for x in xmin..=xmax {
            for y in ymin..=ymax {
                if Self::tile_center(x, y).dist(at) <= r {
                    if let Some(event) = self.set(x, y, occluded) {
                        events.push(event);
                    }
                }
            }
        }

        SceneEvent::EventSet(events)
    }

    pub fn set_active(&mut self, active: bool) -> Option<SceneEvent> {
        if self.active == active {
            None
        } else {
            let old = self.active;
            self.active = active;
            Some(SceneEvent::FogActive(old, self.active))
        }
    }

    pub fn nearest_clear(&self, x: u32, y: u32) -> (u32, u32) {
        if !self.active {
            return (x, y);
        }

        let mut tiles = vec![(x, y)];
        let add_tile = |tiles: &mut Vec<(u32, u32)>, p: (u32, u32)| {
            if !tiles.contains(&p) {
                tiles.push(p);
            }
        };
        let add_adjacent = |tiles: &mut Vec<(u32, u32)>, (x, y): (u32, u32)| {
            let xm = x > 0;
            let ym = y > 0;
            let xp = x < self.w - 1;
            let yp = y < self.h - 1;

            if xm {
                if ym {
                    add_tile(tiles, (x - 1, y - 1));
                }
                add_tile(tiles, (x - 1, y));
                if yp {
                    add_tile(tiles, (x - 1, y + 1));
                }
            }

            if ym {
                add_tile(tiles, (x, y - 1));
            }

            if yp {
                add_tile(tiles, (x, y + 1));
            }

            if xp {
                if ym {
                    add_tile(tiles, (x + 1, y - 1));
                }
                add_tile(tiles, (x + 1, y));
                if yp {
                    add_tile(tiles, (x + 1, y + 1));
                }
            }
        };

        let mut i = 0;
        while let Some(&(x, y)) = tiles.get(i) {
            if !self.occluded(x, y) {
                return (x, y);
            }
            add_adjacent(&mut tiles, (x, y));
            i += 1;
        }
        (x, y)
    }
}

#[cfg(test)]
mod test {
    use super::Fog;

    #[test]
    fn test_reveal() {
        let mut fog = Fog::new(43, 25);
        assert!(fog.occluded(10, 12));
        fog.reveal(45, 30); // Should be a nop
        assert!(fog.occluded(42, 24));
        fog.reveal(42, 24);
        assert!(!fog.occluded(42, 24));
    }

    #[test]
    fn test_occlude() {
        let mut fog = Fog::new(8, 8);
        assert!(fog.occluded(3, 3));
        fog.reveal(3, 3);
        assert!(!fog.occluded(3, 3));
        fog.occlude(3, 3);
        assert!(fog.occluded(3, 3));
    }

    #[test]
    fn test_resize() {
        let mut fog = Fog::new(5, 5);
        fog.reveal(3, 3);
        assert!(!fog.occluded(3, 3));
        fog.resize(142, 123);
        assert!(!fog.occluded(3, 3));
        assert!(fog.occluded(123, 111));
        fog.reveal(123, 111);
        assert!(!fog.occluded(123, 111));
    }
}
