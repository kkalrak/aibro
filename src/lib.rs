mod agent;
mod api;
mod runtime;

#[cfg(target_os = "windows")]
#[unsafe(no_mangle)]
unsafe extern "C" fn RunWinMain(
    instance: cef::sys::HINSTANCE,
    _: *const u8,
    _: i32,
    sandbox: *mut u8,
) -> i32 {
    match std::panic::catch_unwind(|| runtime::run(cef::MainArgs { instance }, sandbox)) {
        Ok(Ok(())) => 0,
        Ok(Err(error)) => {
            eprintln!("aibro: {error:#}");
            1
        }
        Err(_) => 1,
    }
}
