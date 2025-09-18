#![windows_subsystem = "windows"]

use anyhow::Result;

fn main() -> Result<()> {
    // GUI entrypoint: no console will be created for this binary.
    // Delegate initialization to the shared crate code.
    aoba::init_common();
    if let Err(err) = aoba::start_gui() {
        std::fs::write(
            std::env::temp_dir().join("aoba_gui_error.log"),
            format!("GUI start error: {err:#?}"),
        )?;
        println!(
            "AOBA GUI failed to start, details written to {:?}",
            std::env::temp_dir().join("aoba_gui_error.log")
        );
    }

    Ok(())
}
