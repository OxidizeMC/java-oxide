#[macro_export]
macro_rules! io_data_error {
    ($($arg:tt)*) => {{
        let message: String = format!($($arg)*);
        std::io::Error::new(std::io::ErrorKind::InvalidData, message)
    }};
}

#[macro_export]
macro_rules! io_data_err {
    ($($arg:tt)*) => { Err($crate::io_data_error!($($arg)*)) };
}

mod difference;
mod generated_file;
mod progress;

pub use difference::Difference;
pub use generated_file::write_generated;
pub use progress::Progress;
