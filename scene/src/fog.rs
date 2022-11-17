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
    pub w: u32,
    pub h: u32,
    fog: Vec<u32>,
}

impl Fog {
    const BITS: u32 = 32;

    pub fn new(w: u32, h: u32) -> Fog {
        Fog {
            w,
            h,
            fog: Self::make_fog(w, h),
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

    pub fn reveal(&mut self, x: u32, y: u32) {
        if !self.on_map(x, y) {
            return;
        }

        let index = self.idx(x, y);
        if let Some(row) = self.fog.get_mut(index) {
            *row |= 1 << (x % Self::BITS);
        }
    }

    pub fn occlude(&mut self, x: u32, y: u32) {
        if !self.on_map(x, y) {
            return;
        }

        let index = self.idx(x, y);
        if let Some(row) = self.fog.get_mut(index) {
            *row &= !(1 << (x % Self::BITS));
        }
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
