//! Uniform-bucket spatial index for entity queries (nearby, vision, smell) without full scans.
//! One instance per entity kind. Rebuilt each tick from positions in index order — cheap at
//! current scales and trivially deterministic; incremental maintenance is a legal Stage 8
//! optimization. Not serialized; rebuilt after load.

pub const BUCKET: u32 = 8; // world cells per bucket side

#[derive(Clone, Default)]
pub struct SpatialIndex {
    bw: u32,
    bh: u32,
    buckets: Vec<Vec<u32>>, // entity arena indices, pushed in index order
}

impl SpatialIndex {
    pub fn new(world_w: u32, world_h: u32) -> SpatialIndex {
        let bw = (world_w + BUCKET - 1) / BUCKET;
        let bh = (world_h + BUCKET - 1) / BUCKET;
        SpatialIndex { bw, bh, buckets: vec![Vec::new(); (bw * bh) as usize] }
    }
    pub fn clear(&mut self) {
        for b in &mut self.buckets {
            b.clear();
        }
    }
    #[inline]
    fn bucket_of(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 {
            return None;
        }
        let bx = x as u32 / BUCKET;
        let by = y as u32 / BUCKET;
        if bx >= self.bw || by >= self.bh {
            None
        } else {
            Some((by * self.bw + bx) as usize)
        }
    }
    #[inline]
    pub fn insert(&mut self, idx: u32, x: i32, y: i32) {
        if let Some(b) = self.bucket_of(x, y) {
            self.buckets[b].push(idx);
        }
    }
    /// Visit all entities within Chebyshev radius `r` of (x,y), in deterministic order
    /// (bucket row-major, insertion order within bucket). Callback filters by exact distance.
    pub fn for_each_near<F: FnMut(u32)>(&self, x: i32, y: i32, r: i32, mut f: F) {
        let bx0 = ((x - r).max(0) as u32) / BUCKET;
        let by0 = ((y - r).max(0) as u32) / BUCKET;
        let bx1 = ((x + r).max(0) as u32) / BUCKET;
        let by1 = ((y + r).max(0) as u32) / BUCKET;
        for by in by0..=by1.min(self.bh - 1) {
            for bx in bx0..=bx1.min(self.bw - 1) {
                for &e in &self.buckets[(by * self.bw + bx) as usize] {
                    f(e);
                }
            }
        }
    }
}
