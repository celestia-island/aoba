/// TUI UI E2E test suite - Pure UI element testing
///
/// This suite tests UI rendering, layout, and visual elements using terminal capture.
/// Does NOT test business logic - that's handled by the separate tui_e2e suite.
use anyhow::Result;
use clap::Parser;

mod ui_basic;

use ui_basic::test_ui_basic_navigation;

/// TUI UI E2E test suite with selective test execution
#[derive(Parser, Debug)]
#[command(name = "tui_ui_e2e")]
#[command(about = "TUI UI E2E test suite - UI element testing only", long_about = None)]
struct Args {
    /// Enable debug mode (show debug breakpoints and additional logging)
    #[arg(long)]
    debug: bool,

    /// Run only test 0: Basic navigation
    #[arg(long)]
    test0: bool,
}

impl Args {
    /// Check if any specific test is selected
    fn has_specific_tests(&self) -> bool {
        self.test0
    }

    /// Check if a specific test should run
    fn should_run_test(&self, test_num: usize) -> bool {
        if !self.has_specific_tests() {
            // If no specific tests selected, run all tests
            return true;
        }

        match test_num {
            0 => self.test0,
            _ => false,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    // Set debug mode if requested
    if args.debug {
        std::env::set_var("DEBUG_MODE", "1");
        log::info!("🐛 Debug mode enabled");
    }

    log::info!("🧪 Starting TUI UI E2E Tests...");
    log::info!("📍 Testing UI elements, rendering, and visual feedback");

    // Test 0: Basic navigation and page rendering
    if args.should_run_test(0) {
        log::info!("🧪 Test 0/1: Basic navigation and page rendering");
        test_ui_basic_navigation().await?;
    }

    log::info!("🧪 All selected TUI UI E2E tests passed!");

    Ok(())
}
