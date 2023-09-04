cfg_if::cfg_if! {
    if #[cfg(windows)] {
        mod windows;
        pub use self::windows::*;
    } else {
        compile_error!("This platform is not supported");
    }
}
