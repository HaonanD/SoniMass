use vergen::EmitBuilder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    EmitBuilder::builder()
        .git_sha(true)    // true = short (7-char) SHA
        .git_dirty(false) // emits VERGEN_GIT_DIRTY = "true" / "false"
        .emit()?;
    Ok(())
}
