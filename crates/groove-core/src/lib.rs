//! Nền của Groovex: không UI. Mọi `mod` khai sẵn ở groove-gd1_01, task sau không sửa tệp này.

pub mod cli;
pub mod i18n;
pub mod keymap;
pub mod logging;
pub mod model;
pub mod paths;
pub mod queue;
pub mod session;
pub mod settings;
#[cfg(any(test, feature = "test-support"))]
pub mod test_dir;
pub mod timeline;
