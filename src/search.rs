use std::{collections::HashMap, ffi::c_int};

use crate::{
    BiomeID, Dimension, Dimension_DIM_OVERWORLD, Generator, MCVersion, MCVersion_MC_NEWEST, Pos,
    Range, StructureType, applySeed, genBiomes, getMinCacheSize, getSpawn, getStructurePos,
    setupGenerator,
};

pub const SPAWN: &str = "spawn";

/// 搜索参照点：出生点，或之前某个命名步骤的位置
#[derive(Clone)]
pub enum Origin {
    Spawn,
    Named(String),
}

/// 步骤类型：找结构 或 找生物群系
#[derive(Clone)]
pub enum StepKind {
    Structure(StructureType),
    Biome(BiomeID),
}

/// 一个搜索步骤
#[derive(Clone)]
pub struct Step {
    pub name: String,
    pub origin: Origin,
    pub radius: i32,
    pub kind: StepKind,
}

impl Step {
    pub fn new(name: impl Into<String>, origin: Origin, radius: i32, kind: StepKind) -> Self {
        Self {
            name: name.into(),
            origin,
            radius,
            kind,
        }
    }

    pub fn new_structure(
        name: impl Into<String>,
        origin: Origin,
        radius: i32,
        st: StructureType,
    ) -> Self {
        Self::new(name, origin, radius, StepKind::Structure(st))
    }

    pub fn new_biome(name: impl Into<String>, origin: Origin, radius: i32, b: BiomeID) -> Self {
        Self::new(name, origin, radius, StepKind::Biome(b))
    }
}

pub struct Searcher {
    pub steps: Vec<Step>,
}

pub struct SearchConfig {
    pub mc: MCVersion,
    pub world: Dimension,
    pub max_match: usize,
    pub step_region: usize, // 建筑搜索步进
    pub scale: i32,         // 区块匹配采样率
    pub height: i32,        // 高度
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            mc: MCVersion_MC_NEWEST,
            world: Dimension_DIM_OVERWORLD,
            max_match: 10,
            step_region: 512,
            scale: 4,
            height: 63, // 海平面
        }
    }
}

impl Searcher {
    pub fn new(steps: Vec<Step>) -> Self {
        Self { steps }
    }

    pub fn matchs(&self, cfg: &SearchConfig, seed: u64) -> Option<HashMap<String, (i32, i32)>> {
        unsafe {
            let mut g: Generator = std::mem::zeroed();
            setupGenerator(&mut g, cfg.mc as i32, 0);
            applySeed(&mut g, cfg.world as i32, seed);

            let spawn = getSpawn(&mut g);

            let mut binds = HashMap::new();
            binds.insert(SPAWN.to_string(), (spawn.x, spawn.z));

            for step in &self.steps {
                let center = match &step.origin {
                    Origin::Spawn => (spawn.x, spawn.z),
                    Origin::Named(n) => match binds.get(n) {
                        Some(p) => *p,
                        None => return None,
                    },
                };

                let found = match &step.kind {
                    StepKind::Structure(st) => {
                        find_structure_near(seed, center, step.radius, cfg, *st)
                    }
                    StepKind::Biome(b) => find_biome_near(&mut g, center, step.radius, cfg, *b),
                };

                match found {
                    Some(p) => {
                        binds.insert(step.name.clone(), p);
                    }
                    None => return None,
                }
            }

            Some(binds)
        }
    }
}

/// 在 center 周围 radius 格内查找结构
fn find_structure_near(
    seed: u64,
    center: (i32, i32),
    radius: i32,
    cfg: &SearchConfig,
    st: StructureType,
) -> Option<(i32, i32)> {
    let step_region = cfg.step_region as i32;
    // 结构以 512 方块为一个 region 生成，遍历覆盖范围的 region
    let min_rx = (center.0 - radius).div_euclid(step_region);
    let max_rx = (center.0 + radius).div_euclid(step_region);
    let min_rz = (center.1 - radius).div_euclid(step_region);
    let max_rz = (center.1 + radius).div_euclid(step_region);

    let r2 = radius * radius;

    for rx in min_rx..=max_rx {
        for rz in min_rz..=max_rz {
            unsafe {
                let mut pos: Pos = std::mem::zeroed();
                let get = getStructurePos(st as c_int, cfg.mc as c_int, seed, rx, rz, &mut pos);

                if get == 0 {
                    continue;
                }

                let dx = pos.x - center.0;
                let dz = pos.z - center.1;
                if dx * dx + dz * dz > r2 {
                    continue;
                }

                return Some((pos.x, pos.z));
            }
        }
    }
    None
}

fn find_biome_near(
    g: &mut Generator,
    center: (i32, i32),
    radius: i32,
    cfg: &SearchConfig,
    st: BiomeID,
) -> Option<(i32, i32)> {
    let scale = cfg.scale as i32;

    let cxs = center.0.div_euclid(scale);
    let czs = center.1.div_euclid(scale);

    let rs = radius.div_euclid(scale) + 2;

    // Range 描述一个 cuboid：以 (cxs-rs, czs-rs) 为起点，尺寸 (2rs+1)^2
    let r = Range {
        scale,
        x: cxs - rs,
        z: czs - rs,
        sx: rs * 2 + 1,
        sz: rs * 2 + 1,
        y: cfg.height, // 采样层（Y=63 通常在地表）
        sy: 1,
    };

    let buf_len = unsafe { getMinCacheSize(g, scale, r.sx, r.sy, r.sz) };
    // 注意：必须是 buf_len 个 i32 的缓冲区。
    // 原写法 vec![0, buf_len as i32] 只是长度为 2 的 Vec，
    // genBiomes 写入时会越界破坏堆，运行时报 "malloc(): corrupted top size"。
    let mut cache: Vec<c_int> = vec![0; buf_len.max(0) as usize];
    let rc = unsafe { genBiomes(g, cache.as_mut_ptr(), r) };
    if rc != 0 {
        return None;
    }

    let r2 = radius * radius;

    for dz in 0..r.sz {
        for dx in 0..r.sx {
            let bx = (r.x + dx) * scale;
            let bz = (r.z + dz) * scale;

            let ddx = bx - center.0;
            let ddz = bz - center.1;
            if ddx * ddx + ddz * ddz > r2 {
                continue;
            }

            let idx = (dz * r.sx + dx) as usize;
            if cache[idx] == st as i32 {
                return Some((bx, bz));
            }
        }
    }
    None
}
