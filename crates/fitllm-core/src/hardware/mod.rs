//! Hardware profiler. STUB — replaced by the hardware agent with real
//! cross-platform detection. Keeps the crate compiling in the meantime.

use crate::types::*;
use anyhow::Result;

pub fn profile() -> Result<HardwareProfile> {
    Ok(HardwareProfile {
        cpu: Cpu {
            model: "unknown".into(),
            vendor: "unknown".into(),
            physical_cores: 0,
            logical_cores: 0,
            base_clock_mhz: None,
            max_clock_mhz: None,
            flags: CpuFlags::default(),
        },
        gpus: vec![],
        apple_silicon: None,
        memory: Memory { total_mb: 0, available_mb: 0 },
        storage: Storage { models_dir: String::new(), free_mb: 0, read_mbps: None },
        os: Os { name: "unknown".into(), version: String::new(), arch: String::new() },
        backends: vec!["cpu".into()],
        detected_at: String::new(),
    })
}
