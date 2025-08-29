#![windows_subsystem = "windows"]

fn main() {
    // GUI entrypoint: no console will be created for this binary.
    // Delegate initialization to the shared crate code.
    aoba::init_common();
    if let Err(e) = aoba::start_gui() {
        let _ = std::fs::write(
            std::env::temp_dir().join("aoba_gui_error.log"),
            format!("GUI start error: {:#?}", e),
        );
        println!(
            "AOBA GUI failed to start, details written to {:?}",
            std::env::temp_dir().join("aoba_gui_error.log")
        );
    }
}
