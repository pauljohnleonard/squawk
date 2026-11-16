use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::anyhow;
use eframe::{self, egui};
use egui_plot::{Line, Plot, PlotBounds, PlotPoints};

use crate::{
    ControlBlock, Instrument, MidiLearnState, ParameterId,
    analysis::{AudioTap, PitchEstimate, estimate_pitch_nccf},
    settings::SettingsStore,
};
use squawk_engine::{
    ExciterMode, HANDPAN_DING_MAX, HANDPAN_DING_MIN, HANDPAN_SCALE_COUNT, handpan_notes,
    handpan_scale_name, support_part_flags,
};

pub fn launch(
    control: Arc<ControlBlock>,
    midi_learn: Arc<MidiLearnState>,
    settings: Arc<SettingsStore>,
    audio_tap: Arc<AudioTap>,
) -> anyhow::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1040.0, 780.0])
            .with_min_inner_size([880.0, 640.0]),
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
    /// Handpan rows choose center, between, and shoulder instead of loudness.
    keyboard_rows_set_place: bool,
    keyboard_octave_shift: i8,
    keyboard_velocity: u8,
    keyboard_key_to_note: HashMap<egui::Key, u8>,
    keyboard_order: Vec<egui::Key>,
    /// The quote key has no egui name. This drops the second text event from one press.
    quote_key_at: Option<Instant>,
    /// The module open in the inspector. Not saved.
    open_module: DeskModule,

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
        let keyboard_rows_set_place = settings.snapshot().keyboard_rows_set_place;

        Self {
            control,
            midi_learn,
            settings,
            audio_tap,

            keyboard_midi_enabled: true,
            keyboard_rows_set_place,
            keyboard_octave_shift: 0,
            keyboard_velocity: 100,
            keyboard_key_to_note: HashMap::new(),
            keyboard_order: Vec::new(),
            quote_key_at: None,
            open_module: DeskModule::Handpan,

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
                    egui::Event::Text(text)
                        if Instrument::from_u8(self.control.instrument())
                            == Instrument::Handpan
                            && (text == "'" || text == "\"") =>
                    {
                        let now = Instant::now();
                        if self
                            .quote_key_at
                            .is_some_and(|at| now.duration_since(at) < Duration::from_millis(40))
                        {
                            continue;
                        }
                        self.quote_key_at = Some(now);
                        let notes = handpan_notes(
                            self.control.handpan_scale(),
                            self.control.handpan_ding(),
                        );
                        let note = notes[HANDPAN_SOFT_KEYS.len() - 1];
                        let (velocity, place) = if self.keyboard_rows_set_place {
                            (self.keyboard_velocity, 0.5)
                        } else {
                            (scaled_velocity(self.keyboard_velocity, 0.55), 0.0)
                        };
                        crate::note_on(&self.control, note, velocity, place);
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

                        let handpan_keys =
                            Instrument::from_u8(self.control.instrument()) == Instrument::Handpan;

                        match (key, pressed, handpan_keys) {
                            (egui::Key::ArrowUp, true, false) => {
                                self.shift_octave(1);
                                continue;
                            }
                            (egui::Key::ArrowDown, true, false) => {
                                self.shift_octave(-1);
                                continue;
                            }
                            (egui::Key::ArrowRight, true, _) => {
                                self.shift_velocity(5);
                                continue;
                            }
                            (egui::Key::ArrowLeft, true, _) => {
                                self.shift_velocity(-5);
                                continue;
                            }
                            _ => {}
                        }

                        if handpan_keys {
                            let notes = handpan_notes(
                                self.control.handpan_scale(),
                                self.control.handpan_ding(),
                            );
                            if let Some((note, strength)) = handpan_key(*key, &notes) {
                                let (velocity, place) = if self.keyboard_rows_set_place {
                                    (self.keyboard_velocity, handpan_place(*key))
                                } else {
                                    (scaled_velocity(self.keyboard_velocity, strength), 0.0)
                                };
                                if *pressed {
                                    if self.keyboard_key_to_note.contains_key(key) {
                                        continue;
                                    }
                                    self.keyboard_key_to_note.insert(*key, note);
                                    crate::note_on(&self.control, note, velocity, place);
                                } else if self.keyboard_key_to_note.remove(key).is_some() {
                                    crate::note_off(&self.control, note);
                                }
                            }
                            continue;
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
                                crate::note_on(&self.control, note, self.keyboard_velocity, 0.0);
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
                                        0.0,
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

    fn shift_octave(&mut self, delta: i8) {
        self.keyboard_octave_shift = (self.keyboard_octave_shift + delta).clamp(-4, 4);
    }

    fn shift_velocity(&mut self, delta: i16) {
        let next = self.keyboard_velocity as i16 + delta;
        self.keyboard_velocity = next.clamp(1, 127) as u8;
    }

    fn draw_ding_wash(&self, ui: &mut egui::Ui) {
        ui.label("Wash");
        let mut wash = self.control.ding_wash();
        let previous = wash;
        let selected = match wash {
            0 => "Resonator",
            2 => "Tambura",
            _ => "Waveshape",
        };
        egui::ComboBox::from_id_source("ding_wash")
            .selected_text(selected)
            .show_ui(ui, |combo| {
                combo.selectable_value(&mut wash, 0, "Resonator");
                combo.selectable_value(&mut wash, 1, "Waveshape");
                combo.selectable_value(&mut wash, 2, "Tambura");
            });
        if wash != previous {
            self.control.set_ding_wash(wash);
            if let Err(err) = self.settings.update_ding_wash(wash) {
                eprintln!("Warning: Failed to persist Ding wash: {err}");
            }
        }
        let params = if wash == 2 {
            ParameterId::tambura()
        } else {
            ParameterId::ding_wash()
        };
        for &param in params {
            self.draw_parameter(ui, param, true);
            ui.add_space(8.0);
        }
        ui.label("Formant");
        for param in [ParameterId::FormantRate, ParameterId::FormantDepth] {
            self.draw_parameter(ui, param, true);
            ui.add_space(8.0);
        }
        for &param in ParameterId::formant() {
            self.draw_parameter(ui, param, true);
            ui.add_space(8.0);
        }
    }

    fn reconcile_module(&mut self) {
        let handpan = Instrument::from_u8(self.control.instrument()) == Instrument::Handpan;
        if handpan && self.open_module == DeskModule::Modal {
            self.open_module = DeskModule::Handpan;
        } else if !handpan && self.open_module != DeskModule::Modal {
            self.open_module = DeskModule::Modal;
        }
    }

    fn strips(&self) -> &'static [DeskModule] {
        if Instrument::from_u8(self.control.instrument()) == Instrument::Handpan {
            &HANDPAN_STRIPS
        } else {
            &MODAL_STRIPS
        }
    }

    fn draw_input_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|row| {
            row.label("Instrument");
            let mut instrument = self.control.instrument();
            let selected = match Instrument::from_u8(instrument) {
                Instrument::Handpan => "Handpan",
                Instrument::Modal => "Modal",
            };
            egui::ComboBox::from_id_source("instrument")
                .selected_text(selected)
                .show_ui(row, |combo: &mut egui::Ui| {
                    combo.selectable_value(&mut instrument, Instrument::Modal.as_u8(), "Modal");
                    combo.selectable_value(&mut instrument, Instrument::Handpan.as_u8(), "Handpan");
                });
            if instrument != self.control.instrument() {
                self.control.set_instrument_u8(instrument);
                self.open_module = if Instrument::from_u8(instrument) == Instrument::Handpan {
                    DeskModule::Handpan
                } else {
                    DeskModule::Modal
                };
                if let Err(err) = self.settings.update_instrument(instrument) {
                    eprintln!("Warning: Failed to persist instrument update: {err}");
                }
            }

            row.checkbox(&mut self.keyboard_midi_enabled, "Keyboard");
            row.label("Velocity");
            row.push_id("velocity_down", |row| {
                if row.button("-").clicked() {
                    self.shift_velocity(-5);
                }
            });
            row.label(format!("{}", self.keyboard_velocity));
            row.push_id("velocity_up", |row| {
                if row.button("+").clicked() {
                    self.shift_velocity(5);
                }
            });
            if Instrument::from_u8(self.control.instrument()) != Instrument::Handpan {
                row.add_space(12.0);
                row.label("Octave");
                row.push_id("octave_down", |row| {
                    if row.button("-").clicked() {
                        self.shift_octave(-1);
                    }
                });
                row.label(format!("{}", self.keyboard_octave_shift));
                row.push_id("octave_up", |row| {
                    if row.button("+").clicked() {
                        self.shift_octave(1);
                    }
                });
            }
            row.add_space(12.0);
            if row.button("Reset to defaults").clicked() {
                self.control.reset_parameters();
                self.keyboard_rows_set_place = false;
                if let Err(err) = self.settings.reset_parameters() {
                    eprintln!("Warning: Failed to persist parameter reset: {err}");
                }
            }
        });

        if Instrument::from_u8(self.control.instrument()) == Instrument::Handpan {
            let notes = handpan_notes(self.control.handpan_scale(), self.control.handpan_ding());
            let mut place = self.keyboard_rows_set_place;
            if ui
                .checkbox(&mut place, "Rows set where the strike lands")
                .changed()
            {
                self.keyboard_rows_set_place = place;
                if let Err(err) = self.settings.update_keyboard_rows_set_place(place) {
                    eprintln!("Warning: Failed to persist strike-place setting: {err}");
                }
            }
            ui.label(format!(
                "One column, one pad: {}. Octave is not used.",
                note_list(&notes)
            ));
            if self.keyboard_rows_set_place {
                ui.label("Z X C V B N M , . /     center");
                ui.label("S D F G H J K L ; '     between");
                ui.label("E R T Y U I O P [ ]     shoulder");
            } else {
                ui.label("Z X C V B N M , . /     loud");
                ui.label("S D F G H J K L ; '     softer");
                ui.label("E R T Y U I O P [ ]     softest");
            }
        } else {
            ui.label(
                "Notes: A W S E D F T G Y H U J K, then O L P ; for the upper notes. Up/down arrows change octave, left/right change velocity.",
            );
        }
    }

    fn draw_desk(&mut self, ui: &mut egui::Ui, fader_h: f32) {
        let strips = self.strips();
        let count = strips.len() as f32;
        let gap = 8.0;
        let width = ((ui.available_width() - gap * (count - 1.0)) / count).clamp(88.0, 124.0);
        egui::Grid::new("desk_strips")
            .num_columns(strips.len())
            .spacing(egui::vec2(gap, 6.0))
            .min_col_width(width)
            .show(ui, |ui| {
                for &module in strips {
                    self.draw_strip_name(ui, module, width);
                }
                ui.end_row();
                for &module in strips {
                    self.draw_strip_pan(ui, module, width);
                }
                ui.end_row();
                for &module in strips {
                    self.draw_strip_fader(ui, module, width, fader_h);
                }
                ui.end_row();
                for &module in strips {
                    self.draw_strip_mute(ui, module, width);
                }
                ui.end_row();
                for &module in strips {
                    self.draw_strip_send(ui, module, width);
                }
                ui.end_row();
            });
        if Instrument::from_u8(self.control.instrument()) == Instrument::Handpan {
            ui.label(
                egui::RichText::new(
                    "Handpan, Ding, Recent, Second, Bass, and Echo: 1 is today's level.",
                )
                .weak(),
            );
        }
    }

    fn draw_strip_name(&mut self, ui: &mut egui::Ui, module: DeskModule, width: f32) {
        ui.vertical(|ui| {
            ui.set_min_width(width);
            let selected = self.open_module == module;
            if ui
                .add_sized(
                    [width, 22.0],
                    egui::SelectableLabel::new(selected, module.name()),
                )
                .clicked()
            {
                self.open_module = module;
            }
        });
    }

    fn draw_strip_pan(&self, ui: &mut egui::Ui, module: DeskModule, width: f32) {
        ui.vertical(|ui| {
            ui.set_min_width(width);
            if let Some(param) = module.pan() {
                ui.label("Pan");
                self.add_strip_slider(ui, param, false);
                self.draw_strip_meta(ui, param);
            }
        });
    }

    fn draw_strip_fader(&self, ui: &mut egui::Ui, module: DeskModule, width: f32, fader_h: f32) {
        ui.vertical(|ui| {
            ui.set_min_width(width);
            if let Some(param) = module.fader() {
                ui.label("Level");
                ui.vertical_centered(|ui| {
                    ui.spacing_mut().slider_width = fader_h;
                    self.add_strip_slider(ui, param, true);
                });
                self.draw_strip_meta(ui, param);
            }
        });
    }

    fn draw_strip_mute(&self, ui: &mut egui::Ui, module: DeskModule, width: f32) {
        ui.vertical(|ui| {
            ui.set_min_width(width);
            ui.spacing_mut().item_spacing.y = 2.0;
            if let Some(part) = module.part() {
                let muted = self.control.support_muted();
                let solo = self.control.support_solo();
                if ui
                    .selectable_label((muted & (1 << part)) != 0, "Mute")
                    .clicked()
                {
                    self.persist_parts(set_part_bit(muted, part, (muted & (1 << part)) == 0), solo);
                }
                if ui
                    .selectable_label((solo & (1 << part)) != 0, "Solo")
                    .clicked()
                {
                    let muted = self.control.support_muted();
                    self.persist_parts(muted, set_part_bit(solo, part, (solo & (1 << part)) == 0));
                }
            }
        });
    }

    fn draw_strip_send(&self, ui: &mut egui::Ui, module: DeskModule, width: f32) {
        ui.vertical(|ui| {
            ui.set_min_width(width);
            if let Some(param) = module.send() {
                ui.label("Send");
                self.add_strip_slider(ui, param, false);
                self.draw_strip_meta(ui, param);
            }
        });
    }

    fn persist_parts(&self, muted: u8, solo: u8) {
        self.control.set_support_parts(muted, solo);
        if let Err(err) = self
            .settings
            .update_support_parts(support_part_flags(muted), support_part_flags(solo))
        {
            eprintln!("Warning: Failed to persist accompaniment mute and solo: {err}");
        }
    }

    fn add_strip_slider(&self, ui: &mut egui::Ui, param: ParameterId, vertical: bool) {
        if !vertical {
            ui.spacing_mut().slider_width = ui.available_width();
        }
        let mut value = self.control.parameter_value(param);
        let mut slider = egui::Slider::new(&mut value, param.slider_range()).show_value(false);
        if vertical {
            slider = slider.vertical();
        }
        if param.logarithmic() {
            slider = slider.logarithmic(true);
        }
        if param == ParameterId::DingStrings {
            slider = slider.integer();
        }
        let response = ui.add(slider).on_hover_text(param.label());
        if response.changed() {
            let clamped = param.clamp(value);
            self.control.set_parameter_value(param, clamped);
            if let Err(err) = self.settings.update_parameter(param, clamped) {
                eprintln!("Warning: Failed to persist parameter update: {err}");
            }
        }
    }

    fn draw_strip_meta(&self, ui: &mut egui::Ui, param: ParameterId) {
        let value = self.control.parameter_value(param);
        ui.label(param.format_value(value));
        let pending = self.midi_learn.pending();
        let listening = pending == Some(param);
        let other_pending = pending.is_some() && !listening;
        let label = if listening {
            "Stop".to_string()
        } else if let Some(cc) = self.midi_learn.mapping_for(param) {
            format!("CC {cc}")
        } else {
            "Learn".to_string()
        };
        if ui
            .add_enabled(!other_pending, egui::Button::new(label).small())
            .clicked()
        {
            self.toggle_learn(param);
        }
    }

    fn draw_inspector(&self, ui: &mut egui::Ui) {
        ui.heading(self.open_module.name());
        match self.open_module {
            DeskModule::Handpan => {
                draw_handpan_layout(ui, &self.control, &self.settings);
                ui.add_space(6.0);
                for &param in ParameterId::handpan() {
                    self.draw_parameter(ui, param, true);
                    ui.add_space(8.0);
                }
            }
            DeskModule::Support => {
                ui.label(
                    "Pulse and Fade move Ding, Recent, Second, and Bass. Width, Wander, Drift, and Motion move the resonator bands.",
                );
                ui.add_space(6.0);
                for &param in ParameterId::accompaniment() {
                    if param == ParameterId::EchoFeedback {
                        continue;
                    }
                    self.draw_parameter(ui, param, true);
                    ui.add_space(8.0);
                }
            }
            DeskModule::Ding => self.draw_ding_wash(ui),
            DeskModule::Recent => {
                ui.label(
                    "Follows the degree you have been playing most. Its motion lives on Support.",
                );
            }
            DeskModule::Second => {
                ui.label("Follows the next most played degree. Its motion lives on Support.");
            }
            DeskModule::Bass => {
                ui.label("Follows the degree the beat agrees on. Its motion lives on Support.");
            }
            DeskModule::Echo => {
                ui.label("Delayed hits play the Handpan, so they follow its fader, pan, and send.");
                ui.add_space(6.0);
                self.draw_parameter(ui, ParameterId::EchoFeedback, true);
            }
            DeskModule::Reverb => {
                for &param in ParameterId::reverb() {
                    if matches!(
                        param,
                        ParameterId::ReverbHandpan | ParameterId::ReverbSupport
                    ) {
                        continue;
                    }
                    self.draw_parameter(ui, param, true);
                    ui.add_space(8.0);
                }
            }
            DeskModule::Modal => self.draw_modal(ui),
        }
    }

    fn draw_modal(&self, ui: &mut egui::Ui) {
        ui.horizontal(|row| {
            row.label("Exciter");
            let mut mode_u8 = self.control.exciter_mode();
            let selected = match ExciterMode::from_u8(mode_u8) {
                ExciterMode::Strike => "Strike",
                ExciterMode::Breath => "Breath",
            };
            egui::ComboBox::from_id_source("exciter_mode")
                .selected_text(selected)
                .show_ui(row, |combo: &mut egui::Ui| {
                    combo.selectable_value(&mut mode_u8, ExciterMode::Strike.as_u8(), "Strike");
                    combo.selectable_value(&mut mode_u8, ExciterMode::Breath.as_u8(), "Breath");
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

        let breath_mode = ExciterMode::from_u8(self.control.exciter_mode()) == ExciterMode::Breath;
        for &param in ParameterId::modal() {
            if param == ParameterId::OutputGain {
                continue;
            }
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
    }

    fn draw_parameter(&self, ui: &mut egui::Ui, param: ParameterId, enabled: bool) {
        self.add_parameter_slider(ui, param, enabled, true);
        self.draw_parameter_assignment(ui, param, enabled);
    }

    fn add_parameter_slider(
        &self,
        ui: &mut egui::Ui,
        param: ParameterId,
        enabled: bool,
        show_label: bool,
    ) {
        let mut value = self.control.parameter_value(param);
        let mut slider = egui::Slider::new(&mut value, param.slider_range());
        if show_label {
            slider = slider.text(param.label());
        }
        if param.logarithmic() {
            slider = slider.logarithmic(true);
        }
        if param == ParameterId::DingStrings {
            slider = slider.integer();
        }
        if ui.add_enabled(enabled, slider).changed() {
            let clamped = param.clamp(value);
            self.control.set_parameter_value(param, clamped);
            if let Err(err) = self.settings.update_parameter(param, clamped) {
                eprintln!("Warning: Failed to persist parameter update: {err}");
            }
        }
    }

    fn draw_parameter_assignment(&self, ui: &mut egui::Ui, param: ParameterId, enabled: bool) {
        let value = self.control.parameter_value(param);
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
                self.toggle_learn(param);
            }

            match self.midi_learn.mapping_for(param) {
                Some(cc) => row.label(format!("CC {cc}")),
                None => row.label("Unassigned"),
            };
        });
    }

    fn toggle_learn(&self, param: ParameterId) {
        if self.midi_learn.pending() == Some(param) {
            let _ = self.midi_learn.take_pending();
            return;
        }
        let removed = self.midi_learn.clear_mapping(param);
        if removed && let Err(err) = self.settings.remove_mapping(param) {
            eprintln!(
                "Warning: Failed to persist MIDI unlearn for {}: {err}",
                param.label()
            );
        }
        self.midi_learn.request(param);
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

    /// Pitch debug view. It stays off the desk.
    #[allow(dead_code)]
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
        self.reconcile_module();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("squawk");
            self.draw_input_bar(ui);
            ui.add_space(4.0);
            self.draw_peak_meter(ui);
            if let Some(param) = self.midi_learn.pending() {
                ui.colored_label(
                    egui::Color32::LIGHT_GREEN,
                    format!("Listening for CC assignment for {}", param.label()),
                );
            }
            ui.separator();
            let available = ui.available_height();
            let inspector_h = (available * 0.40).clamp(100.0, 360.0);
            let mixer_h = (available - inspector_h).max(0.0);
            let fader_h = (mixer_h - 250.0).clamp(56.0, 120.0);
            egui::ScrollArea::vertical()
                .id_source("desk")
                .auto_shrink([false, false])
                .max_height(mixer_h)
                .show(ui, |ui| {
                    self.draw_desk(ui, fader_h);
                });
            ui.separator();
            let inspector_h = ui.available_height();
            egui::ScrollArea::vertical()
                .id_source("inspector")
                .auto_shrink([false, false])
                .max_height(inspector_h)
                .show(ui, |ui| {
                    self.draw_inspector(ui);
                });
        });
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DeskModule {
    Handpan,
    Support,
    Ding,
    Recent,
    Second,
    Bass,
    Echo,
    Reverb,
    Modal,
}

const HANDPAN_STRIPS: [DeskModule; 8] = [
    DeskModule::Handpan,
    DeskModule::Support,
    DeskModule::Ding,
    DeskModule::Recent,
    DeskModule::Second,
    DeskModule::Bass,
    DeskModule::Echo,
    DeskModule::Reverb,
];

const MODAL_STRIPS: [DeskModule; 1] = [DeskModule::Modal];

impl DeskModule {
    fn name(self) -> &'static str {
        match self {
            DeskModule::Handpan => "Handpan",
            DeskModule::Support => "Support",
            DeskModule::Ding => "Ding",
            DeskModule::Recent => "Recent",
            DeskModule::Second => "Second",
            DeskModule::Bass => "Bass",
            DeskModule::Echo => "Echo",
            DeskModule::Reverb => "Reverb",
            DeskModule::Modal => "Modal",
        }
    }

    fn fader(self) -> Option<ParameterId> {
        match self {
            DeskModule::Handpan => Some(ParameterId::MixHandpan),
            DeskModule::Support => Some(ParameterId::SupportLevel),
            DeskModule::Ding => Some(ParameterId::MixDing),
            DeskModule::Recent => Some(ParameterId::MixRecent),
            DeskModule::Second => Some(ParameterId::MixSecond),
            DeskModule::Bass => Some(ParameterId::MixBass),
            DeskModule::Echo => Some(ParameterId::MixTaps),
            DeskModule::Modal => Some(ParameterId::OutputGain),
            DeskModule::Reverb => None,
        }
    }

    fn pan(self) -> Option<ParameterId> {
        match self {
            DeskModule::Handpan => Some(ParameterId::PanHandpan),
            DeskModule::Ding => Some(ParameterId::PanDing),
            DeskModule::Recent => Some(ParameterId::PanRecent),
            DeskModule::Second => Some(ParameterId::PanSecond),
            DeskModule::Bass => Some(ParameterId::PanBass),
            DeskModule::Support | DeskModule::Echo | DeskModule::Reverb | DeskModule::Modal => None,
        }
    }

    fn send(self) -> Option<ParameterId> {
        match self {
            DeskModule::Handpan => Some(ParameterId::ReverbHandpan),
            DeskModule::Support => Some(ParameterId::ReverbSupport),
            DeskModule::Ding
            | DeskModule::Recent
            | DeskModule::Second
            | DeskModule::Bass
            | DeskModule::Echo
            | DeskModule::Reverb
            | DeskModule::Modal => None,
        }
    }

    fn part(self) -> Option<usize> {
        match self {
            DeskModule::Ding => Some(0),
            DeskModule::Recent => Some(1),
            DeskModule::Second => Some(2),
            DeskModule::Bass => Some(3),
            DeskModule::Echo => Some(4),
            DeskModule::Handpan | DeskModule::Support | DeskModule::Reverb | DeskModule::Modal => {
                None
            }
        }
    }
}

fn set_part_bit(mask: u8, index: usize, on: bool) -> u8 {
    let bit = 1u8 << index;
    if on { mask | bit } else { mask & !bit }
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

const HANDPAN_LOUD_KEYS: [egui::Key; 10] = [
    egui::Key::Z,
    egui::Key::X,
    egui::Key::C,
    egui::Key::V,
    egui::Key::B,
    egui::Key::N,
    egui::Key::M,
    egui::Key::Comma,
    egui::Key::Period,
    egui::Key::Slash,
];
const HANDPAN_SOFT_KEYS: [egui::Key; 10] = [
    egui::Key::S,
    egui::Key::D,
    egui::Key::F,
    egui::Key::G,
    egui::Key::H,
    egui::Key::J,
    egui::Key::K,
    egui::Key::L,
    egui::Key::Semicolon,
    egui::Key::Backslash,
];
const HANDPAN_SOFTEST_KEYS: [egui::Key; 10] = [
    egui::Key::E,
    egui::Key::R,
    egui::Key::T,
    egui::Key::Y,
    egui::Key::U,
    egui::Key::I,
    egui::Key::O,
    egui::Key::P,
    egui::Key::OpenBracket,
    egui::Key::CloseBracket,
];

/// `(midi note, strength)` where strength is 1, 0.55, or 0.30 of the velocity setting.
fn handpan_key(key: egui::Key, notes: &[u8]) -> Option<(u8, f32)> {
    for (index, candidate) in HANDPAN_LOUD_KEYS.iter().enumerate() {
        if key == *candidate {
            return Some((notes[index], 1.0));
        }
    }
    for (index, candidate) in HANDPAN_SOFT_KEYS.iter().enumerate() {
        if key == *candidate {
            return Some((notes[index], 0.55));
        }
    }
    for (index, candidate) in HANDPAN_SOFTEST_KEYS.iter().enumerate() {
        if key == *candidate {
            return Some((notes[index], 0.30));
        }
    }
    None
}

fn note_name(note: u8) -> String {
    let names = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    format!("{}{}", names[(note % 12) as usize], (note as i16 / 12) - 1)
}

fn draw_handpan_layout(
    ui: &mut egui::Ui,
    control: &crate::ControlBlock,
    settings: &crate::SettingsStore,
) {
    let mut scale = control.handpan_scale();
    let mut ding = control.handpan_ding();
    let mut tuning = control.handpan_tuning();
    let previous = (scale, ding, tuning);

    ui.horizontal(|row| {
        row.label("Scale");
        egui::ComboBox::from_id_source("handpan_scale")
            .selected_text(handpan_scale_name(scale))
            .show_ui(row, |combo| {
                for index in 0..HANDPAN_SCALE_COUNT {
                    combo.selectable_value(&mut scale, index, handpan_scale_name(index));
                }
            });
        row.label("Ding");
        egui::ComboBox::from_id_source("handpan_ding")
            .selected_text(note_name(ding))
            .show_ui(row, |combo| {
                for note in HANDPAN_DING_MIN..=HANDPAN_DING_MAX {
                    combo.selectable_value(&mut ding, note, note_name(note));
                }
            });
        row.label("Tuning");
        egui::ComboBox::from_id_source("handpan_tuning")
            .selected_text(if tuning == 0 { "Equal" } else { "Just" })
            .show_ui(row, |combo| {
                combo.selectable_value(&mut tuning, 0, "Equal");
                combo.selectable_value(&mut tuning, 1, "Just");
            });
    });

    if (scale, ding, tuning) != previous {
        control.set_handpan_layout(scale, ding, tuning);
        if let Err(err) = settings.update_handpan_layout(scale, ding, tuning) {
            eprintln!("Warning: Failed to persist handpan layout: {err}");
        }
    }

    let notes = handpan_notes(scale, ding);
    ui.label(format!(
        "{}  {}  ·  {}",
        handpan_scale_name(scale),
        note_list(&notes),
        if tuning == 0 { "Equal" } else { "Just" }
    ));
}

fn note_list(notes: &[u8]) -> String {
    notes
        .iter()
        .copied()
        .map(note_name)
        .collect::<Vec<_>>()
        .join(" ")
}

/// 0 at the center row, 0.5 between, 1 on the shoulder row.
fn handpan_place(key: egui::Key) -> f32 {
    if HANDPAN_LOUD_KEYS.contains(&key) {
        0.0
    } else if HANDPAN_SOFT_KEYS.contains(&key) {
        0.5
    } else {
        1.0
    }
}

fn scaled_velocity(master: u8, strength: f32) -> u8 {
    ((master as f32) * strength).round().clamp(1.0, 127.0) as u8
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handpan_rows_share_pads_and_get_softer() {
        let pads = handpan_notes(0, 50);
        let loud = handpan_key(egui::Key::Z, &pads).unwrap();
        let soft = handpan_key(egui::Key::S, &pads).unwrap();
        let softest = handpan_key(egui::Key::E, &pads).unwrap();
        assert_eq!(loud.0, 50);
        assert_eq!(soft.0, loud.0);
        assert_eq!(softest.0, loud.0);
        assert!(loud.1 > soft.1 && soft.1 > softest.1);

        let notes: Vec<u8> = HANDPAN_LOUD_KEYS
            .into_iter()
            .map(|key| handpan_key(key, &pads).unwrap().0)
            .collect();
        assert_eq!(notes, pads.to_vec());
        let unique = {
            let mut sorted = notes.clone();
            sorted.sort_unstable();
            sorted.dedup();
            sorted.len()
        };
        assert_eq!(unique, 10);
        assert_eq!(
            handpan_key(egui::Key::Period, &pads).unwrap().0,
            handpan_key(egui::Key::Semicolon, &pads).unwrap().0
        );
        assert!(
            handpan_key(egui::Key::Slash, &pads).unwrap().0
                > handpan_key(egui::Key::Comma, &pads).unwrap().0
        );

        assert_eq!(
            handpan_key(egui::Key::Comma, &pads).unwrap().0,
            handpan_key(egui::Key::L, &pads).unwrap().0
        );
        assert_eq!(
            handpan_key(egui::Key::P, &pads).unwrap().0,
            handpan_key(egui::Key::Comma, &pads).unwrap().0
        );
        assert!(handpan_key(egui::Key::A, &pads).is_none());
        assert_eq!(scaled_velocity(100, 0.55), 55);
        assert_eq!(scaled_velocity(100, 0.30), 30);
        assert_eq!(handpan_place(egui::Key::Z), 0.0);
        assert_eq!(handpan_place(egui::Key::S), 0.5);
        assert_eq!(handpan_place(egui::Key::E), 1.0);
        assert_eq!(handpan_place(egui::Key::P), 1.0);
    }
}
