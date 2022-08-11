// Prebuild most shapes as there's no need to recompute common shapes every
// time they're needed.
const CIRCLE_EDGES: u32 = 32;
const RECTANGLE: &'static [f32] = &[0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0];

// Returns points for a regular polygon with n edges.
//
// Note the resultant shape will be oriented with the first vertex at the
// top center of the tile, i.e. a 4gon is a diamond and not a square.
pub fn ngon(n: u32) -> Vec<f32> {
    // n sides, each side a triangle of 3 points, each point 2 floats
    let mut coords = Vec::with_capacity((n * 3 * 2) as usize);

    let r = 0.5;
    let dt = 2.0 * std::f32::consts::PI / n as f32;

    let mut add_point = |(x, y)| {
        coords.push(x + r);
        coords.push(y + r);
    };

    let c = (0.0, 0.0);
    let mut prev = None;
    for i in 0..=n {
        let theta = i as f32 * dt;
        let q = (r * theta.cos(), r * theta.sin());
        if let Some(p) = prev {
            add_point(p);
            add_point(q);
            add_point(c);
        }
        prev = Some(q);
    }

    coords
}

pub fn circle() -> Vec<f32> {
    ngon(CIRCLE_EDGES)
}

pub fn rectangle() -> &'static [f32] {
    RECTANGLE
}
