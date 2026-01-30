//! Main application state and UI

use eframe::egui;
use std::sync::mpsc::{channel, Receiver};
use std::thread;

use crate::analysis::{
    parse_fasta, run_screening, AlignmentData, AnalysisMethod, AnalysisParams,
    LengthResult, ProgressUpdate, ScreeningResults,
};

/// Application state
pub struct OligoScreenApp {
    // Input tab state
    fasta_input: String,
    alignment_data: Option<AlignmentData>,
    input_error: Option<String>,

    // Analysis parameters
    params: AnalysisParams,
    method_selection: MethodSelection,

    // Analysis state
    is_analyzing: bool,
    analysis_progress: Option<ProgressUpdate>,
    progress_rx: Option<Receiver<ProgressUpdate>>,
    results_rx: Option<Receiver<ScreeningResults>>,

    // Results state
    results: Option<ScreeningResults>,
    selected_length: Option<u32>,
    selected_position: Option<usize>,
    show_detail_window: bool,

    // View state
    current_tab: Tab,
    zoom_level: f32,

    // Save/Load
    save_error: Option<String>,
    load_error: Option<String>,

    // Deferred actions
    pending_save: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Input,
    Analysis,
    Results,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MethodSelection {
    NoAmbiguities,
    FixedAmbiguities,
    Incremental,
}

impl Default for OligoScreenApp {
    fn default() -> Self {
        Self {
            fasta_input: String::new(),
            alignment_data: None,
            input_error: None,
            params: AnalysisParams::default(),
            method_selection: MethodSelection::NoAmbiguities,
            is_analyzing: false,
            analysis_progress: None,
            progress_rx: None,
            results_rx: None,
            results: None,
            selected_length: None,
            selected_position: None,
            show_detail_window: false,
            current_tab: Tab::Input,
            zoom_level: 1.0,
            save_error: None,
            load_error: None,
            pending_save: false,
        }
    }
}

impl OligoScreenApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self::default()
    }

    fn parse_input(&mut self) {
        self.input_error = None;
        self.alignment_data = None;

        if self.fasta_input.trim().is_empty() {
            return;
        }

        match parse_fasta(&self.fasta_input) {
            Ok(data) => {
                self.alignment_data = Some(data);
            }
            Err(e) => {
                self.input_error = Some(e);
            }
        }
    }

    fn start_analysis(&mut self) {
        let Some(data) = &self.alignment_data else {
            return;
        };

        // Update method from selection
        self.params.method = match self.method_selection {
            MethodSelection::NoAmbiguities => AnalysisMethod::NoAmbiguities,
            MethodSelection::FixedAmbiguities => {
                AnalysisMethod::FixedAmbiguities(self.params.method.get_fixed_ambiguities())
            }
            MethodSelection::Incremental => {
                AnalysisMethod::Incremental(self.params.method.get_incremental_pct())
            }
        };

        let data_clone = data.clone();
        let params_clone = self.params.clone();

        let (progress_tx, progress_rx) = channel();
        let (results_tx, results_rx) = channel();

        self.progress_rx = Some(progress_rx);
        self.results_rx = Some(results_rx);
        self.is_analyzing = true;
        self.analysis_progress = None;

        thread::spawn(move || {
            let results = run_screening(&data_clone, &params_clone, Some(progress_tx));
            let _ = results_tx.send(results);
        });
    }

    fn check_analysis_progress(&mut self) {
        // Check for progress updates
        if let Some(rx) = &self.progress_rx {
            while let Ok(progress) = rx.try_recv() {
                self.analysis_progress = Some(progress);
            }
        }

        // Check for results
        if let Some(rx) = &self.results_rx {
            if let Ok(results) = rx.try_recv() {
                self.selected_length = results.results_by_length.keys().min().copied();
                self.results = Some(results);
                self.is_analyzing = false;
                self.progress_rx = None;
                self.results_rx = None;
                self.current_tab = Tab::Results;
            }
        }
    }

    fn save_results(&mut self) {
        let Some(results) = &self.results else {
            self.save_error = Some("No results to save".to_string());
            return;
        };

        if let Some(path) = rfd::FileDialog::new()
            .add_filter("JSON", &["json"])
            .set_file_name("screening_results.json")
            .save_file()
        {
            match serde_json::to_string_pretty(results) {
                Ok(json) => {
                    if let Err(e) = std::fs::write(&path, json) {
                        self.save_error = Some(format!("Failed to write file: {}", e));
                    } else {
                        self.save_error = None;
                    }
                }
                Err(e) => {
                    self.save_error = Some(format!("Failed to serialize: {}", e));
                }
            }
        }
    }

    fn load_results(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("JSON", &["json"])
            .pick_file()
        {
            match std::fs::read_to_string(&path) {
                Ok(json) => match serde_json::from_str::<ScreeningResults>(&json) {
                    Ok(results) => {
                        self.selected_length = results.results_by_length.keys().min().copied();
                        self.results = Some(results);
                        self.load_error = None;
                        self.current_tab = Tab::Results;
                    }
                    Err(e) => {
                        self.load_error = Some(format!("Failed to parse: {}", e));
                    }
                },
                Err(e) => {
                    self.load_error = Some(format!("Failed to read file: {}", e));
                }
            }
        }
    }

    fn load_fasta_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("FASTA", &["fasta", "fa", "fna", "fas", "txt"])
            .pick_file()
        {
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    self.fasta_input = content;
                    self.parse_input();
                }
                Err(e) => {
                    self.input_error = Some(format!("Failed to read file: {}", e));
                }
            }
        }
    }
}

impl AnalysisMethod {
    fn get_fixed_ambiguities(&self) -> u32 {
        match self {
            AnalysisMethod::FixedAmbiguities(n) => *n,
            _ => 1,
        }
    }

    fn get_incremental_pct(&self) -> u32 {
        match self {
            AnalysisMethod::Incremental(n) => *n,
            _ => 50,
        }
    }
}

impl eframe::App for OligoScreenApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Check for analysis completion
        if self.is_analyzing {
            self.check_analysis_progress();
            ctx.request_repaint();
        }

        // Handle deferred save action
        if self.pending_save {
            self.pending_save = false;
            self.save_results();
        }

        // Top menu bar
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Load FASTA...").clicked() {
                        self.load_fasta_file();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Load Results...").clicked() {
                        self.load_results();
                        ui.close_menu();
                    }
                    if ui.button("Save Results...").clicked() {
                        self.save_results();
                        ui.close_menu();
                    }
                });
            });
        });

        // Tab bar
        egui::TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.current_tab, Tab::Input, "Input Data");
                ui.selectable_value(&mut self.current_tab, Tab::Analysis, "Analysis Setup");
                ui.selectable_value(&mut self.current_tab, Tab::Results, "Results");
            });
        });

        // Status bar
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if self.is_analyzing {
                    ui.spinner();
                    if let Some(ref progress) = self.analysis_progress {
                        ui.label(&progress.message);
                    } else {
                        ui.label("Starting analysis...");
                    }
                } else if let Some(ref results) = self.results {
                    ui.label(format!(
                        "Results: {} sequences, {} bp alignment",
                        results.total_sequences, results.alignment_length
                    ));
                } else if let Some(ref data) = self.alignment_data {
                    ui.label(format!(
                        "Loaded: {} sequences, {} bp alignment",
                        data.len(),
                        data.alignment_length
                    ));
                } else {
                    ui.label("Load FASTA data to begin");
                }
            });
        });

        // Main content
        egui::CentralPanel::default().show(ctx, |ui| {
            match self.current_tab {
                Tab::Input => self.show_input_tab(ui),
                Tab::Analysis => self.show_analysis_tab(ui),
                Tab::Results => self.show_results_tab(ui),
            }
        });

        // Detail window
        if self.show_detail_window {
            self.show_variant_detail_window(ctx);
        }
    }
}

impl OligoScreenApp {
    fn show_input_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Input Data");
        ui.separator();

        ui.horizontal(|ui| {
            if ui.button("Load FASTA File").clicked() {
                self.load_fasta_file();
            }
            if ui.button("Clear").clicked() {
                self.fasta_input.clear();
                self.alignment_data = None;
                self.input_error = None;
            }
            if ui.button("Load Example").clicked() {
                self.fasta_input = EXAMPLE_FASTA.to_string();
                self.parse_input();
            }
        });

        ui.add_space(10.0);

        // Input text area
        ui.label("Paste or load aligned sequences (FASTA format):");

        let available_height = ui.available_height() - 150.0;
        egui::ScrollArea::vertical()
            .max_height(available_height.max(200.0))
            .show(ui, |ui| {
                let response = ui.add(
                    egui::TextEdit::multiline(&mut self.fasta_input)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY)
                        .desired_rows(20),
                );

                if response.changed() {
                    self.parse_input();
                }
            });

        ui.add_space(10.0);

        // Status/error display
        if let Some(ref error) = self.input_error {
            ui.colored_label(egui::Color32::RED, format!("Error: {}", error));
        }

        // Quality report
        if let Some(ref data) = self.alignment_data {
            ui.group(|ui| {
                ui.heading("Alignment Summary");
                ui.horizontal(|ui| {
                    ui.label(format!("Sequences: {}", data.len()));
                    ui.separator();
                    ui.label(format!("Alignment length: {} bp", data.alignment_length));
                });
            });
        }
    }

    fn show_analysis_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Analysis Setup");
        ui.separator();

        // Check if data is loaded
        if self.alignment_data.is_none() {
            ui.colored_label(
                egui::Color32::YELLOW,
                "Please load FASTA data in the Input tab first.",
            );
            return;
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            // Analysis method selection
            ui.group(|ui| {
                ui.heading("Analysis Method");

                ui.radio_value(
                    &mut self.method_selection,
                    MethodSelection::NoAmbiguities,
                    "No Ambiguities - Find all unique exact variants",
                );

                ui.horizontal(|ui| {
                    ui.radio_value(
                        &mut self.method_selection,
                        MethodSelection::FixedAmbiguities,
                        "Fixed Ambiguities - Use up to N ambiguity codes per variant",
                    );
                });

                if self.method_selection == MethodSelection::FixedAmbiguities {
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);
                        ui.label("Max ambiguities:");
                        let mut n = self.params.method.get_fixed_ambiguities();
                        if ui.add(egui::DragValue::new(&mut n).range(0..=20)).changed() {
                            self.params.method = AnalysisMethod::FixedAmbiguities(n);
                        }
                    });
                }

                ui.horizontal(|ui| {
                    ui.radio_value(
                        &mut self.method_selection,
                        MethodSelection::Incremental,
                        "Incremental - Find variants covering X% of remaining sequences",
                    );
                });

                if self.method_selection == MethodSelection::Incremental {
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);
                        ui.label("Target coverage per step (%):");
                        let mut pct = self.params.method.get_incremental_pct();
                        if ui.add(egui::DragValue::new(&mut pct).range(1..=100)).changed() {
                            self.params.method = AnalysisMethod::Incremental(pct);
                        }
                    });
                }
            });

            ui.add_space(10.0);

            // Global options
            ui.group(|ui| {
                ui.heading("Global Options");
                ui.checkbox(&mut self.params.exclude_n, "Exclude N (any base) as ambiguity code");
            });

            ui.add_space(10.0);

            // Oligo length range
            ui.group(|ui| {
                ui.heading("Oligo Length Range");
                ui.horizontal(|ui| {
                    ui.label("Minimum length:");
                    ui.add(egui::DragValue::new(&mut self.params.min_oligo_length).range(3..=100));
                    ui.add_space(20.0);
                    ui.label("Maximum length:");
                    ui.add(egui::DragValue::new(&mut self.params.max_oligo_length).range(3..=100));
                });

                // Ensure min <= max
                if self.params.min_oligo_length > self.params.max_oligo_length {
                    self.params.max_oligo_length = self.params.min_oligo_length;
                }

                let range = self.params.max_oligo_length - self.params.min_oligo_length + 1;
                if range > 20 {
                    ui.colored_label(
                        egui::Color32::YELLOW,
                        format!("Warning: Large length range ({}) may take significant time", range),
                    );
                }
            });

            ui.add_space(10.0);

            // Resolution
            ui.group(|ui| {
                ui.heading("Analysis Resolution");
                ui.horizontal(|ui| {
                    ui.label("Step size (bases):");
                    ui.add(egui::DragValue::new(&mut self.params.resolution).range(1..=100));
                });
                ui.label("Lower values = more positions analyzed, higher resolution");
            });

            ui.add_space(10.0);

            // Coverage threshold
            ui.group(|ui| {
                ui.heading("Coverage Threshold");
                ui.horizontal(|ui| {
                    ui.label("Target coverage (%):");
                    ui.add(egui::DragValue::new(&mut self.params.coverage_threshold).range(1.0..=100.0));
                });
                ui.label("Number of variants needed to reach this coverage will be reported");
            });

            ui.add_space(20.0);

            // Run button
            ui.horizontal(|ui| {
                let can_run = self.alignment_data.is_some() && !self.is_analyzing;
                if ui.add_enabled(can_run, egui::Button::new("Run Analysis")).clicked() {
                    self.start_analysis();
                }

                if self.is_analyzing {
                    ui.spinner();
                    if let Some(ref progress) = self.analysis_progress {
                        ui.label(&progress.message);
                    }
                }
            });
        });
    }

    fn show_results_tab(&mut self, ui: &mut egui::Ui) {
        if self.results.is_none() {
            ui.heading("Results");
            ui.separator();
            ui.label("No results yet. Run an analysis from the Analysis Setup tab.");
            return;
        }

        // Extract needed data before UI rendering to avoid borrow issues
        let has_results = self.results.is_some();

        ui.horizontal(|ui| {
            ui.heading("Results");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Save Results").clicked() && has_results {
                    self.pending_save = true;
                }
            });
        });
        ui.separator();

        // Get lengths list and other info we need
        let (lengths, _total_seqs, _alignment_len, consensus) = {
            let results = self.results.as_ref().unwrap();
            let mut lengths: Vec<u32> = results.results_by_length.keys().copied().collect();
            lengths.sort();
            (
                lengths,
                results.total_sequences,
                results.alignment_length,
                results.consensus_sequence.clone(),
            )
        };

        // Length selector
        ui.horizontal(|ui| {
            ui.label("Oligo Length:");

            egui::ComboBox::from_id_salt("length_selector")
                .selected_text(
                    self.selected_length
                        .map(|l| format!("{} bp", l))
                        .unwrap_or_else(|| "Select...".to_string()),
                )
                .show_ui(ui, |ui| {
                    for length in &lengths {
                        ui.selectable_value(
                            &mut self.selected_length,
                            Some(*length),
                            format!("{} bp", length),
                        );
                    }
                });

            ui.add_space(20.0);
            ui.label("Zoom:");
            ui.add(egui::Slider::new(&mut self.zoom_level, 0.5..=3.0));
        });

        ui.add_space(10.0);

        // Results display
        if let Some(length) = self.selected_length {
            // Clone the length result to avoid borrow issues
            let length_result = self.results.as_ref()
                .and_then(|r| r.results_by_length.get(&length))
                .cloned();

            if let Some(length_result) = length_result {
                let coverage_threshold = self.results.as_ref()
                    .map(|r| r.params.coverage_threshold)
                    .unwrap_or(95.0);
                self.show_length_results(ui, &length_result, &consensus, coverage_threshold);
            }
        }

        // Error messages
        if let Some(ref error) = self.save_error {
            ui.colored_label(egui::Color32::RED, error);
        }
        if let Some(ref error) = self.load_error {
            ui.colored_label(egui::Color32::RED, error);
        }
    }

    fn show_length_results(
        &mut self,
        ui: &mut egui::Ui,
        length_result: &LengthResult,
        consensus: &str,
        coverage_threshold: f64,
    ) {
        let positions = &length_result.positions;
        if positions.is_empty() {
            ui.label("No positions analyzed for this length.");
            return;
        }

        // Summary stats
        let non_skipped: Vec<_> = positions.iter().filter(|p| !p.analysis.skipped).collect();
        let min_variants = non_skipped.iter().map(|p| p.variants_needed).min().unwrap_or(0);
        let max_variants = non_skipped.iter().map(|p| p.variants_needed).max().unwrap_or(0);
        let avg_variants: f64 = if non_skipped.is_empty() {
            0.0
        } else {
            non_skipped.iter().map(|p| p.variants_needed).sum::<usize>() as f64
                / non_skipped.len() as f64
        };

        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(format!("Positions analyzed: {}", positions.len()));
                ui.separator();
                ui.label(format!("Min variants: {}", min_variants));
                ui.separator();
                ui.label(format!("Max variants: {}", max_variants));
                ui.separator();
                ui.label(format!("Avg variants: {:.1}", avg_variants));
            });
        });

        ui.add_space(10.0);

        // Consensus sequence display (scrollable)
        ui.group(|ui| {
            ui.label("Consensus sequence:");
            egui::ScrollArea::horizontal().show(ui, |ui| {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(consensus).monospace().size(12.0 * self.zoom_level),
                    )
                    .wrap_mode(egui::TextWrapMode::Extend),
                );
            });
        });

        ui.add_space(10.0);

        // Heat map / bar visualization
        ui.label(format!("Variants needed to reach {:.0}% coverage:", coverage_threshold));

        let available_width = ui.available_width();
        let bar_width = (available_width / positions.len() as f32).max(4.0) * self.zoom_level;

        egui::ScrollArea::horizontal()
            .id_salt("results_scroll")
            .show(ui, |ui| {
                // Position numbers row
                ui.horizontal(|ui| {
                    for pos_result in positions {
                        let response = ui.allocate_response(
                            egui::vec2(bar_width, 20.0),
                            egui::Sense::hover(),
                        );

                        if response.hovered() {
                            let pos_text = format!("{}", pos_result.position + 1);
                            ui.painter().text(
                                response.rect.center(),
                                egui::Align2::CENTER_CENTER,
                                &pos_text,
                                egui::FontId::proportional(10.0 * self.zoom_level),
                                egui::Color32::WHITE,
                            );
                        }
                    }
                });

                // Bar chart
                ui.horizontal(|ui| {
                    let max_height = 100.0 * self.zoom_level;
                    let max_for_scale = max_variants.max(10) as f32;

                    for pos_result in positions {
                        let bar_height = if pos_result.analysis.skipped {
                            2.0
                        } else {
                            ((pos_result.variants_needed as f32 / max_for_scale) * max_height)
                                .max(2.0)
                        };

                        let color = if pos_result.analysis.skipped {
                            egui::Color32::DARK_GRAY
                        } else {
                            variant_count_color(pos_result.variants_needed)
                        };

                        let response = ui.allocate_response(
                            egui::vec2(bar_width, max_height),
                            egui::Sense::click(),
                        );

                        // Draw bar from bottom
                        let bar_rect = egui::Rect::from_min_size(
                            egui::pos2(
                                response.rect.min.x,
                                response.rect.max.y - bar_height,
                            ),
                            egui::vec2(bar_width - 1.0, bar_height),
                        );

                        ui.painter().rect_filled(bar_rect, 0.0, color);

                        // Hover effect
                        if response.hovered() {
                            ui.painter().rect_stroke(
                                bar_rect,
                                0.0,
                                egui::Stroke::new(2.0, egui::Color32::WHITE),
                                egui::StrokeKind::Outside,
                            );
                        }

                        // Click to show details
                        if response.clicked() {
                            self.selected_position = Some(pos_result.position);
                            self.show_detail_window = true;
                        }

                        // Tooltip
                        response.on_hover_ui(|ui| {
                            ui.label(format!("Position: {}", pos_result.position + 1));
                            if pos_result.analysis.skipped {
                                ui.label(format!(
                                    "Skipped: {}",
                                    pos_result.analysis.skip_reason.as_deref().unwrap_or("Unknown")
                                ));
                            } else {
                                ui.label(format!(
                                    "Variants needed: {}",
                                    pos_result.variants_needed
                                ));
                                ui.label(format!(
                                    "Coverage: {:.1}%",
                                    pos_result.analysis.coverage_at_threshold
                                ));
                                ui.label(format!(
                                    "Sequences analyzed: {}",
                                    pos_result.analysis.sequences_analyzed
                                ));
                            }
                            ui.label("Click for details");
                        });
                    }
                });

                // Variant count labels row
                ui.horizontal(|ui| {
                    for pos_result in positions {
                        let response = ui.allocate_response(
                            egui::vec2(bar_width, 20.0),
                            egui::Sense::hover(),
                        );

                        let text = if pos_result.analysis.skipped {
                            "-".to_string()
                        } else {
                            format!("{}", pos_result.variants_needed)
                        };

                        ui.painter().text(
                            response.rect.center(),
                            egui::Align2::CENTER_CENTER,
                            &text,
                            egui::FontId::proportional(9.0 * self.zoom_level),
                            egui::Color32::LIGHT_GRAY,
                        );
                    }
                });
            });

        // Legend
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.label("Legend:");
            ui.add_space(10.0);

            let legend_colors = [
                (egui::Color32::from_rgb(0, 180, 0), "1 (best)"),
                (egui::Color32::from_rgb(150, 180, 0), "2-5"),
                (egui::Color32::from_rgb(255, 165, 0), "6-10"),
                (egui::Color32::from_rgb(220, 50, 50), ">10"),
                (egui::Color32::DARK_GRAY, "skipped"),
            ];

            for (color, label) in legend_colors {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(15.0, 15.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 2.0, color);
                ui.label(label);
                ui.add_space(10.0);
            }
        });
    }

    fn show_variant_detail_window(&mut self, ctx: &egui::Context) {
        let Some(ref results) = self.results else {
            self.show_detail_window = false;
            return;
        };

        let Some(length) = self.selected_length else {
            self.show_detail_window = false;
            return;
        };

        let Some(position) = self.selected_position else {
            self.show_detail_window = false;
            return;
        };

        let Some(length_result) = results.results_by_length.get(&length) else {
            self.show_detail_window = false;
            return;
        };

        let Some(pos_result) = length_result
            .positions
            .iter()
            .find(|p| p.position == position)
        else {
            self.show_detail_window = false;
            return;
        };

        // Clone data we need for the window
        let pos_result = pos_result.clone();
        let coverage_threshold = results.params.coverage_threshold;

        egui::Window::new(format!("Position {} Details", position + 1))
            .open(&mut self.show_detail_window)
            .default_width(600.0)
            .default_height(400.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("Position: {}", position + 1));
                    ui.separator();
                    ui.label(format!("Oligo length: {} bp", length));
                });

                ui.separator();

                if pos_result.analysis.skipped {
                    ui.colored_label(
                        egui::Color32::YELLOW,
                        format!(
                            "This window was skipped: {}",
                            pos_result.analysis.skip_reason.as_deref().unwrap_or("Unknown reason")
                        ),
                    );
                    return;
                }

                ui.label(format!(
                    "Total sequences: {}",
                    pos_result.analysis.total_sequences
                ));
                ui.label(format!(
                    "Sequences analyzed: {}",
                    pos_result.analysis.sequences_analyzed
                ));
                ui.label(format!(
                    "Variants needed for {:.0}% coverage: {}",
                    coverage_threshold, pos_result.variants_needed
                ));
                ui.label(format!(
                    "Coverage at threshold: {:.1}%",
                    pos_result.analysis.coverage_at_threshold
                ));

                ui.separator();
                ui.heading("Variants");

                egui::ScrollArea::vertical()
                    .max_height(300.0)
                    .show(ui, |ui| {
                        egui::Grid::new("variants_grid")
                            .striped(true)
                            .min_col_width(50.0)
                            .show(ui, |ui| {
                                ui.strong("#");
                                ui.strong("Sequence");
                                ui.strong("Count");
                                ui.strong("Percentage");
                                ui.strong("Cumulative");
                                ui.end_row();

                                let mut cumulative = 0.0;
                                for (i, variant) in
                                    pos_result.analysis.variants.iter().enumerate()
                                {
                                    cumulative += variant.percentage;

                                    let is_threshold = i + 1 == pos_result.variants_needed;

                                    if is_threshold {
                                        ui.colored_label(
                                            egui::Color32::GREEN,
                                            format!("{}", i + 1),
                                        );
                                    } else {
                                        ui.label(format!("{}", i + 1));
                                    }

                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(&variant.sequence)
                                                .monospace()
                                                .size(11.0),
                                        )
                                        .wrap_mode(egui::TextWrapMode::Extend),
                                    );

                                    ui.label(format!("{}", variant.count));
                                    ui.label(format!("{:.1}%", variant.percentage));

                                    if is_threshold {
                                        ui.colored_label(
                                            egui::Color32::GREEN,
                                            format!("{:.1}%", cumulative),
                                        );
                                    } else {
                                        ui.label(format!("{:.1}%", cumulative));
                                    }

                                    ui.end_row();
                                }
                            });
                    });
            });
    }
}

/// Get color for variant count (green = good, red = bad)
fn variant_count_color(count: usize) -> egui::Color32 {
    if count == 0 {
        return egui::Color32::DARK_GRAY;
    }

    if count == 1 {
        egui::Color32::from_rgb(0, 180, 0) // Green - best
    } else if count <= 5 {
        egui::Color32::from_rgb(150, 180, 0) // Yellow-green
    } else if count <= 10 {
        egui::Color32::from_rgb(255, 165, 0) // Orange
    } else {
        egui::Color32::from_rgb(220, 50, 50) // Red - worst
    }
}

const EXAMPLE_FASTA: &str = r#">Seq1
ACGTACGTACGTACGTACGTACGTACGT
>Seq2
ACGTACGTACGTACGTACGTACGTACGT
>Seq3
ACGAACGTACGTACGTACGTACGTACGT
>Seq4
ACGTACGTACGTACGTACGTACGTACGT
>Seq5
ACGTACGAACGTACGTACGTACGTACGT
>Seq6
ACGTACGTACGTACGTACGTACGTACGT
>Seq7
ACGTACGTACGAACGTACGTACGTACGT
>Seq8
ACGTACGTACGTACGTACGTACGTACGT
>Seq9
ACGTACGTACGTACGAACGTACGTACGT
>Seq10
ACGTACGTACGTACGTACGTACGTACGT
"#;
