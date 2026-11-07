//! Os nomes de série, no vocabulário de `metrics.name` (§3.2).
//!
//! Constantes, e não literais espalhados: é este módulo que decide o que é uma
//! série de dispositivo, e o `process_result` consulta a mesma lista.

pub const CPU_USAGE: &str = "cpu_usage";
pub const MEMORY_USAGE: &str = "memory_usage";
pub const MEMORY_USED_BYTES: &str = "memory_used_bytes";
pub const MEMORY_TOTAL_BYTES: &str = "memory_total_bytes";
pub const STORAGE_USAGE: &str = "storage_usage";
pub const LOAD_AVERAGE_1M: &str = "load_average_1m";
pub const PROCESS_MEMORY_BYTES: &str = "process_memory_bytes";
pub const UPTIME_SECONDS: &str = "uptime_seconds";
pub const IN_BPS: &str = "inBps";
pub const OUT_BPS: &str = "outBps";
