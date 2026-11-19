fn main() -> anyhow::Result<()> {
    let path =
        cef::build_util::win::build_bundle(std::path::Path::new("target/bundle"), "aibro", false)?;
    println!("{}", path.display());
    Ok(())
}
