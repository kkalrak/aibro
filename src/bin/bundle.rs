fn main() -> anyhow::Result<()> {
    let release = std::env::args().skip(1).any(|arg| arg == "--release");
    let path = cef::build_util::win::build_bundle(
        std::path::Path::new("target/bundle"),
        "aibro",
        release,
    )?;
    println!("{}", path.display());
    Ok(())
}
