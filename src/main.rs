//! OligoScreen VR - Primer Site Screening Tool
//!
//! A Rust application for screening large sets of aligned DNA sequences
//! to find suitable primer sites with low variability.

mod analysis;
mod app;

use app::OligoScreenApp;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([900.0, 600.0])
            .with_title("OligoScreen VR - Primer Site Screening Tool"),
        ..Default::default()
    };

    eframe::run_native(
        "OligoScreen VR",
        native_options,
        Box::new(|cc| Ok(Box::new(OligoScreenApp::new(cc)))),
    )
}
