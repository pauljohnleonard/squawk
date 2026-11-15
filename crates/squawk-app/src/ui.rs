use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::anyhow;
use eframe::{self, egui};
use egui_plot::{Line, Plot, PlotBounds, PlotPoints};

use crate::{
    ControlBlock, MidiLearnState, ParameterId,
    analysis::{AudioTap, PitchEstimate, estimate_pitch_nccf},
    settings::SettingsStore,
};
use squawk_engine::ExciterMode;

pub fn launch(
    control: Arc<ControlBlock>,
    midi_learn: Arc<MidiLearnState>,
    settings: Arc<SettingsStore>,
    audio_tap: Arc<AudioTap>,
) -> anyhow::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([860.0, 560.0])
            .with_min_inner_size([640.0, 420.0]),
        ..Default::default()
    };

    eframe::run_native(
        "squawk controls",
        native_options,
        Box::new(move |_cc| {
            Box::new(SquawkApp::new(
                control.clone(),
                midi_learn.clone(),
                settings.clone(),
                audio_tap.clone(),
            ))
        }),
    )
    .map_err(|err| anyhow!("eframe error: {err}"))?;

    Ok(())
}

struct SquawkApp {
    control: Arc<ControlBlock>,
    midi_learn: Arc<MidiLearnState>,
    settings: Arc<SettingsStore>,
    audio_tap: Arc<AudioTap>,

    keyboard_midi_enabled: bool,
    keyboard_octave_shift: i8,
    keyboard_velocity: u8,
    keyboard_key_to_note: HashMap<egui::Key, u8>,
    keyboard_order: Vec<egui::Key>,

    analysis_enabled: bool,
    analysis_window_ms: f32,
    analysis_track_around_last_note: bool,
    analysis_update_hz: f32,
    analysis_last_update: Instant,
    analysis_last_estimate: Option<PitchEstimate>,
    analysis_last_note_on_counter: u32,
    analysis_series_start: Instant,
    analysis_plot_enabled: bool,
    analysis_plot_history_s: f32,
    analysis_plot_ignore_initial_s: f32,
    analysis_plot_points: VecDeque<(f64, f64)>,

    analysis_freeze_when_quiet: bool,
    analysis_freeze_rms_threshold: f32,
    analysis_frozen: bool,
    analysis_last_window_rms: f32,
}

impl SquawkApp {
    fn new(
        control: Arc<ControlBlock>,
        midi_learn: Arc<MidiLearnState>,
        settings: Arc<SettingsStore>,
        audio_tap: Arc<AudioTap>,
    ) -> Self {
        audio_tap.set_enabled(false);

        Self {
            control,
            midi_learn,
            settings,
            audio_tap,

            keyboard_midi_enabled: true,
            keyboard_octave_shift: 0,
            keyboard_velocity: 100,
            keyboard_key_to_note: HashMap::new(),
            keyboard_order: Vec::new(),

            analysis_enabled: false,
            analysis_window_ms: 30.0,
            analysis_track_around_last_note: true,
            analysis_update_hz: 10.0,
            analysis_last_update: Instant::now(),
            analysis_last_estimate: None,
            analysis_last_note_on_counter: 0,
            analysis_series_start: Instant::now(),
            analysis_plot_enabled: true,
            analysis_plot_history_s: 3.0,
            analysis_plot_ignore_initial_s: 0.05,
            analysis_plot_points: VecDeque::new(),

            analysis_freeze_when_quiet: true,
            analysis_freeze_rms_threshold: 0.002,
            analysis_frozen: false,
            analysis_last_window_rms: 0.0,
        }
    }

    fn handle_computer_keyboard(&mut self, ctx: &egui::Context) {
        if !self.keyboard_midi_enabled {
            return;
        }

        ctx.input(|i| {
            for event in &i.events {
                match event {
                    egui::Event::WindowFocused(false) => {
                        self.keyboard_key_to_note.clear();
                        self.keyboard_order.clear();
                        crate::note_off(
                            &self.control,
                            self.control
                                .active_note
                                .load(std::sync::atomic::Ordering::Relaxed),
                        );
                    }
                    egui::Event::Key {
                        key,
                        pressed,
                        repeat,
                        ..
                    } => {
                        if *repeat {
                            continue;
                        }

                        match (key, pressed) {
                            (egui::Key::ArrowUp, true) => {
                                self.keyboard_octave_shift =
                                    (self.keyboard_octave_shift + 1).clamp(-4, 4);
                                continue;
                            }
                            (egui::Key::ArrowDown, true) => {
                                self.keyboard_octave_shift =
                                    (self.keyboard_octave_shift - 1).clamp(-4, 4);
                                continue;
                            }
                            (egui::Key::ArrowRight, true) => {
                                self.keyboard_velocity =
                                    (self.keyboard_velocity.saturating_add(5)).clamp(1, 127);
                                continue;
                            }
                            (egui::Key::ArrowLeft, true) => {
                                self.keyboard_velocity =
                                    (self.keyboard_velocity.saturating_sub(5)).clamp(1, 127);
                                continue;
                            }
                            _ => {}
                        }

                        if let Some(note) =
                            computer_key_to_midi_note(*key, self.keyboard_octave_shift)
                        {
                            if *pressed {
                                if self.keyboard_key_to_note.contains_key(key) {
                                    continue;
                                }

                                self.keyboard_key_to_note.insert(*key, note);
                                self.keyboard_order.retain(|k| k != key);
                                self.keyboard_order.push(*key);
                                crate::note_on(&self.control, note, self.keyboard_velocity);
                            } else if let Some(released_note) =
                                self.keyboard_key_to_note.remove(key)
                            {
                                self.keyboard_order.retain(|k| k != key);
                                crate::note_off(&self.control, released_note);

                                if let Some(last_key) = self.keyboard_order.last().copied()
                                    && let Some(last_note) =
                                        self.keyboard_key_to_note.get(&last_key)
                                {
                                    crate::note_on(
                                        &self.control,
                                        *last_note,
                                        self.keyboard_velocity,
                                    );
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        });
    }

    fn draw_parameter(&self, ui: &mut egui::Ui, param: ParameterId, enabled: bool) {
        let mut value = self.control.parameter_value(param);
        let mut slider = egui::Slider::new(&mut value, param.slider_range()).text(param.label());
        if param.logarithmic() {
            slider = slider.logarithmic(true);
        }
        if ui.add_enabled(enabled, slider).changed() {
            let clamped = param.clamp(value);
            self.control.set_parameter_value(param, clamped);
            if let Err(err) = self.settings.update_parameter(param, clamped) {
                eprintln!("Warning: Failed to persist parameter update: {err}");
            }
        }

        ui.horizontal(|row| {
            row.label(param.format_value(value));
            let pending = self.midi_learn.pending();
            let listening = pending == Some(param);
            let other_pending = pending.is_some() && !listening;
            let button_label = if listening {
                "Listening…"
            } else {
                "MIDI Learn"
            };
            if row
                .add_enabled(enabled && !other_pending, egui::Button::new(button_label))
                .clicked()
            {
                if listening {
                    let _ = self.midi_learn.take_pending();
                } else {
                    let removed = self.midi_learn.clear_mapping(param);
                    if removed && let Err(err) = self.settings.remove_mapping(param) {
                        eprintln!(
                            "Warning: Failed to persist MIDI unlearn for {}: {err}",
                            param.label()
                        );
                    }
                    self.midi_learn.request(param);
                }
            }

            match self.midi_learn.mapping_for(param) {
                Some(cc) => row.label(format!("CC {cc}")),
                None => row.label("Unassigned"),
            };
        });
    }

    fn draw_peak_meter(&self, ui: &mut egui::Ui) {
        let peak = self.control.peak_level().clamp(0.0, 1.0);
        let db = if peak <= 0.000_01 {
            "-inf".to_string()
        } else {
            format!("{:.1} dBFS", 20.0 * peak.log10())
        };
        ui.label("Output peak");
        ui.add(
            egui::ProgressBar::new(peak)
                .desired_width(ui.available_width())
                .fill(if peak < 0.9 {
                    egui::Color32::from_rgb(80, 200, 120)
                } else {
                    egui::Color32::from_rgb(220, 90, 90)
                })
                .text(db),
        );
    }

    fn draw_analysis(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        ui.label("Analysis (debug)");

        let mut enabled = self.analysis_enabled;
        if ui.checkbox(&mut enabled, "Estimate pitch (f0)").changed() {
            self.analysis_enabled = enabled;
            self.audio_tap.set_enabled(enabled);
        }

        ui.add_enabled(
            self.analysis_enabled,
            egui::Slider::new(&mut self.analysis_window_ms, 10.0..=120.0).text("window (ms)"),
        );
        ui.add_enabled(
            self.analysis_enabled,
            egui::Checkbox::new(
                &mut self.analysis_track_around_last_note,
                "Track around last Note On",
            ),
        );

        ui.add_enabled(
            self.analysis_enabled,
            egui::Slider::new(&mut self.analysis_update_hz, 2.0..=30.0).text("update rate (Hz)"),
        );

        ui.add_enabled(
            self.analysis_enabled,
            egui::Checkbox::new(&mut self.analysis_plot_enabled, "Plot"),
        );
        ui.add_enabled(
            self.analysis_enabled && self.analysis_plot_enabled,
            egui::Slider::new(&mut self.analysis_plot_history_s, 0.5..=10.0)
                .text("plot history (s)"),
        );
        ui.add_enabled(
            self.analysis_enabled && self.analysis_plot_enabled,
            egui::Slider::new(&mut self.analysis_plot_ignore_initial_s, 0.0..=0.25)
                .text("ignore initial (s)"),
        );

        ui.add_enabled(
            self.analysis_enabled,
            egui::Checkbox::new(&mut self.analysis_freeze_when_quiet, "Freeze when quiet"),
        );
        ui.add_enabled(
            self.analysis_enabled && self.analysis_freeze_when_quiet,
            egui::Slider::new(&mut self.analysis_freeze_rms_threshold, 0.000_5..=0.02)
                .logarithmic(true)
                .text("freeze RMS threshold"),
        );

        if !self.analysis_enabled {
            return;
        }

        // Reset plot + readout series on Note On.
        let note_on_counter = self.control.last_note_on_counter();
        if note_on_counter != self.analysis_last_note_on_counter {
            self.analysis_last_note_on_counter = note_on_counter;
            self.analysis_series_start = Instant::now();
            self.analysis_plot_points.clear();
            self.analysis_last_estimate = None;
            self.analysis_frozen = false;
            // Force an immediate refresh.
            self.analysis_last_update = Instant::now() - Duration::from_secs(10);
        }

        let sr = self.audio_tap.sample_rate_hz();
        let n_samples = ((sr * (self.analysis_window_ms / 1000.0)) as usize).clamp(64, 8192);
        let samples = self.audio_tap.snapshot_latest(n_samples);

        // Always compute window RMS so we can gate/freeze even when the pitch estimator
        // returns None.
        self.analysis_last_window_rms = window_rms(&samples);

        let last_note_hz = self
            .control
            .last_note_on_hz()
            .filter(|f| f.is_finite() && *f > 1.0);
        let expected = if self.analysis_track_around_last_note {
            last_note_hz
        } else {
            None
        };

        // Throttle the estimator so the readout is readable.
        let now = Instant::now();
        let min_dt = Duration::from_secs_f32((1.0 / self.analysis_update_hz).max(0.01));
        if now.duration_since(self.analysis_last_update) >= min_dt {
            self.analysis_last_update = now;

            if self.analysis_freeze_when_quiet
                && !self.analysis_frozen
                && note_on_counter != 0
                && self.analysis_last_window_rms < self.analysis_freeze_rms_threshold
            {
                // Stop updating; hold the graph + readout until the next Note On.
                self.analysis_frozen = true;
            }

            if !self.analysis_frozen {
                self.analysis_last_estimate = estimate_pitch_nccf(&samples, sr, expected);

                if let (Some(est), Some(target)) = (self.analysis_last_estimate, last_note_hz) {
                    let t = now.duration_since(self.analysis_series_start).as_secs_f64();
                    let cents = cents_error(est.f0_hz, target) as f64;
                    self.analysis_plot_points.push_back((t, cents));
                }

                // Drop points older than the requested window.
                let min_t = now.duration_since(self.analysis_series_start).as_secs_f64()
                    - self.analysis_plot_history_s.max(0.1) as f64;
                while let Some((t, _)) = self.analysis_plot_points.front().copied()
                    && t < min_t
                {
                    self.analysis_plot_points.pop_front();
                }
            }
        }

        if self.analysis_frozen {
            ui.colored_label(
                egui::Color32::LIGHT_YELLOW,
                "Frozen (quiet) — waiting for next Note On",
            );
        }

        match self.analysis_last_estimate {
            Some(est) => {
                if let Some(target) = last_note_hz {
                    let cents = cents_error(est.f0_hz, target);
                    ui.label(
                        egui::RichText::new(format!("{:+.1} cents", cents))
                            .size(22.0)
                            .strong(),
                    );
                    let note_text = self
                        .control
                        .last_note_on_note()
                        .map(midi_note_label)
                        .unwrap_or_else(|| "?".to_string());
                    ui.label(format!(
                        "target (last Note On): {}  ({:.1} Hz)",
                        note_text, target
                    ));
                } else {
                    ui.label(egui::RichText::new("(no Note On yet)").italics());
                }

                ui.horizontal(|row| {
                    row.label(format!("measured f0: {:.1} Hz", est.f0_hz));
                    row.label(format!("conf: {:.2}", est.confidence));
                    row.label(format!("rms: {:.4}", est.rms));
                });

                ui.add(
                    egui::ProgressBar::new(est.confidence.clamp(0.0, 1.0))
                        .desired_width(ui.available_width())
                        .text("confidence"),
                );
            }
            None => {
                ui.label("f0: (no estimate)");
            }
        }

        ui.label(format!("window rms: {:.5}", self.analysis_last_window_rms));

        if self.analysis_plot_enabled && last_note_hz.is_some() {
            let points: PlotPoints = self
                .analysis_plot_points
                .iter()
                .map(|(x, y)| [*x, *y])
                .collect();

            let line = Line::new(points).name("cents error");

            let bounds = compute_post_transient_bounds(
                &self.analysis_plot_points,
                self.analysis_plot_ignore_initial_s,
                self.analysis_plot_history_s,
            );

            Plot::new("pitch_cents_plot")
                .height(120.0)
                .y_axis_width(7)
                .y_axis_formatter(|mark, max_chars, range| {
                    format_cents_tick(mark.value, mark.step_size, max_chars, range)
                })
                .include_x(0.0)
                .show(ui, |plot_ui| {
                    if let Some(bounds) = bounds {
                        plot_ui.set_plot_bounds(bounds);
                    }
                    plot_ui.line(line);
                });
        }
    }
}

fn window_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }

    let mut sum_sq = 0.0f32;
    let mut n = 0.0f32;
    for &s in samples {
        if s.is_finite() {
            sum_sq += s * s;
            n += 1.0;
        }
    }
    if n <= 0.0 { 0.0 } else { (sum_sq / n).sqrt() }
}

fn compute_post_transient_bounds(
    points: &VecDeque<(f64, f64)>,
    ignore_initial_s: f32,
    min_x_span_s: f32,
) -> Option<PlotBounds> {
    if points.len() < 2 {
        return None;
    }

    let ignore_t = (ignore_initial_s.max(0.0)) as f64;

    let mut min_y = f64::INFINITY;
    let mut max_y = -f64::INFINITY;
    let mut found = false;
    for (t, y) in points.iter() {
        if *t >= ignore_t {
            min_y = min_y.min(*y);
            max_y = max_y.max(*y);
            found = true;
        }
    }

    if !found {
        for (_, y) in points.iter() {
            min_y = min_y.min(*y);
            max_y = max_y.max(*y);
        }
    }

    if !(min_y.is_finite() && max_y.is_finite()) {
        return None;
    }

    // Ensure a non-zero height.
    if (max_y - min_y).abs() < 1.0e-9 {
        min_y -= 1.0;
        max_y += 1.0;
    }

    // Pad slightly so lines aren't glued to the plot edges.
    let pad = 0.08 * (max_y - min_y);
    min_y -= pad;
    max_y += pad;

    let min_x = 0.0f64;
    let mut max_x = points.back().map(|(t, _)| *t).unwrap_or(0.0);
    max_x = max_x.max(min_x_span_s.max(0.1) as f64);

    let bounds = PlotBounds::from_min_max([min_x, min_y], [max_x, max_y]);
    if bounds.is_valid() {
        Some(bounds)
    } else {
        None
    }
}

fn format_cents_tick(
    value: f64,
    step_size: f64,
    max_chars: usize,
    range: &std::ops::RangeInclusive<f64>,
) -> String {
    let span = (*range.end() - *range.start()).abs().max(1.0e-12);
    let step = step_size.abs().max(1.0e-12);

    // Choose decimals based primarily on step size, but ensure enough precision
    // when zoomed in so we don't end up with labels that all round to "0".
    let mut decimals = (-step.log10()).ceil() as i32;
    if span < 5.0 {
        decimals = decimals.max(1);
    }
    if span < 1.0 {
        decimals = decimals.max(2);
    }
    decimals = decimals.clamp(0, 5);

    let v = if value.abs() < 1.0e-12 { 0.0 } else { value };
    let mut s = format!("{v:+.*}", decimals as usize);

    // If the label is too long for the requested space, fall back to fewer
    // decimals, then scientific.
    if s.len() > max_chars {
        let fewer = (decimals - 1).clamp(0, 3) as usize;
        s = format!("{v:+.*}", fewer);
    }
    if s.len() > max_chars {
        s = format!("{v:+.2e}");
    }
    s
}

impl eframe::App for SquawkApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_millis(16));

        self.handle_computer_keyboard(ctx);

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("squawk mass-spring");
            ui.label("Shape envelopes and resonator couplings or map them to MIDI CCs.");
            ui.separator();

            ui.columns(2, |cols| {
                // Left column: performance + parameters
                cols[0].vertical(|ui| {
                    ui.horizontal(|row| {
                        row.checkbox(
                            &mut self.keyboard_midi_enabled,
                            "Computer keyboard → MIDI notes",
                        );
                        row.label(format!(
                            "octave {}  vel {}",
                            self.keyboard_octave_shift, self.keyboard_velocity
                        ));
                    });
                    ui.label(
                        "Keys: A W S E D F T G Y H U J K (then O L P ; for upper notes)   (↑/↓ octave, ←/→ velocity)",
                    );
                    ui.add_space(6.0);

                    ui.horizontal(|row| {
                        row.label("Exciter");
                        let mut mode_u8 = self.control.exciter_mode();
                        let mode = ExciterMode::from_u8(mode_u8);
                        let selected = match mode {
                            ExciterMode::Strike => "Strike",
                            ExciterMode::Breath => "Breath",
                        };

                        egui::ComboBox::from_id_source("exciter_mode")
                            .selected_text(selected)
                            .show_ui(row, |combo: &mut egui::Ui| {
                                combo.selectable_value(
                                    &mut mode_u8,
                                    ExciterMode::Strike.as_u8(),
                                    "Strike",
                                );
                                combo.selectable_value(
                                    &mut mode_u8,
                                    ExciterMode::Breath.as_u8(),
                                    "Breath",
                                );
                            });

                        if mode_u8 != self.control.exciter_mode() {
                            self.control.set_exciter_mode_u8(mode_u8);
                            if let Err(err) = self.settings.update_exciter_mode(mode_u8) {
                                eprintln!("Warning: Failed to persist exciter mode update: {err}");
                            }
                        }

                        row.label(match ExciterMode::from_u8(mode_u8) {
                            ExciterMode::Breath => "(continuous)",
                            ExciterMode::Strike => "(pulse)",
                        });
                    });

                    if ui.button("Reset parameters").clicked() {
                        self.control.reset_parameters();
                        if let Err(err) = self.settings.reset_parameters() {
                            eprintln!("Warning: Failed to persist parameter reset: {err}");
                        }
                    }

                    ui.separator();

                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            let breath_mode = ExciterMode::from_u8(self.control.exciter_mode())
                                == ExciterMode::Breath;
                            for &param in ParameterId::all() {
                                if param == ParameterId::BreathPressure {
                                    ui.separator();
                                    ui.label("Breath controls (active in Breath mode)");
                                }
                                let enabled = match param {
                                    ParameterId::BreathPressure
                                    | ParameterId::BreathNoiseCutoffHz
                                    | ParameterId::BreathFeedback
                                    | ParameterId::BreathAutoLevel => breath_mode,
                                    _ => true,
                                };

                                self.draw_parameter(ui, param, enabled);
                                ui.add_space(8.0);
                            }
                        });
                });

                // Right column: meters + analysis
                cols[1].vertical(|ui| {
                    self.draw_peak_meter(ui);
                    self.draw_analysis(ui);

                    if let Some(param) = self.midi_learn.pending() {
                        ui.separator();
                        ui.colored_label(
                            egui::Color32::LIGHT_GREEN,
                            format!("Listening for CC assignment for {}", param.label()),
                        );
                    }
                });
            });
        });
    }
}

fn cents_error(measured_hz: f32, target_hz: f32) -> f32 {
    if !(measured_hz.is_finite() && target_hz.is_finite()) {
        return 0.0;
    }
    if measured_hz <= 0.0 || target_hz <= 0.0 {
        return 0.0;
    }
    let ratio = measured_hz / target_hz;
    1200.0 * (ratio.ln() / core::f32::consts::LN_2)
}

fn computer_key_to_midi_note(key: egui::Key, octave_shift: i8) -> Option<u8> {
    let semitone: i16 = match key {
        egui::Key::A => 0,
        egui::Key::W => 1,
        egui::Key::S => 2,
        egui::Key::E => 3,
        egui::Key::D => 4,
        egui::Key::F => 5,
        egui::Key::T => 6,
        egui::Key::G => 7,
        egui::Key::Y => 8,
        egui::Key::H => 9,
        egui::Key::U => 10,
        egui::Key::J => 11,
        egui::Key::K => 12,
        // Upper notes
        egui::Key::O => 13,
        egui::Key::L => 14,
        egui::Key::P => 15,
        egui::Key::Semicolon => 16,
        _ => return None,
    };

    // Base: C4 (MIDI 60)
    let base_note: i16 = 60;
    let shifted = base_note + semitone + (octave_shift as i16 * 12);
    Some(shifted.clamp(0, 127) as u8)
}

fn midi_note_label(note: u8) -> String {
    let names = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let name = names[(note % 12) as usize];
    let octave = (note as i16 / 12) - 1;
    format!("{}{} ({})", name, octave, note)
}
