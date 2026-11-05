//! Adapter Pattern aplicado às famílias de dispositivos.

pub mod contract;
pub mod platforms;
pub mod registry;

pub use contract::{DeviceAccessMethod, DeviceAdapter, DevicePlatform, SyslogConfigurationAdapter};
