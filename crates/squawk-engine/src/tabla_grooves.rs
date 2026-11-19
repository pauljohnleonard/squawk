//! Compile-time catalog of tabla theka patterns (bol → TablaStroke).
//!
//! Add a new groove by defining another `TablaGroove` const and appending it to
//! `GROOVES`. Compound bols (`Dha`, `Dhin`) expand to two simultaneous strokes.

use crate::tabla::TablaStroke;

/// Spoken bol (or rest) used when authoring patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bol {
    Rest,
    Ge,
    Ga,
    Ka,
    Na,
    Tu,
    Ta,
    /// Ge + Na together (sam accent).
    Dha,
    /// Ge + Tu, slightly shorter / softer than Dha.
    Dhin,
}

impl Bol {
    /// Expand into one or two voice triggers: `(stroke, relative amp scale)`.
    pub fn expand(self) -> &'static [(TablaStroke, f32)] {
        match self {
            Bol::Rest => &[],
            Bol::Ge => &[(TablaStroke::Ge, 1.0)],
            Bol::Ga => &[(TablaStroke::Ga, 1.0)],
            Bol::Ka => &[(TablaStroke::Ka, 1.0)],
            Bol::Na => &[(TablaStroke::Na, 1.0)],
            Bol::Tu => &[(TablaStroke::Tu, 1.0)],
            Bol::Ta => &[(TablaStroke::Ta, 1.0)],
            Bol::Dha => &[(TablaStroke::Ge, 1.0), (TablaStroke::Na, 0.9)],
            Bol::Dhin => &[(TablaStroke::Ge, 0.85), (TablaStroke::Tu, 0.8)],
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GrooveHit {
    pub step: u16,
    pub bol: Bol,
    pub amp: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct TablaGroove {
    pub id: &'static str,
    pub name: &'static str,
    pub matras: u16,
    pub steps_per_matra: u16,
    pub hits: &'static [GrooveHit],
}

impl TablaGroove {
    pub fn steps(&self) -> u16 {
        self.matras.saturating_mul(self.steps_per_matra.max(1))
    }

    pub fn hits_at(&self, step: u16) -> impl Iterator<Item = &GrooveHit> {
        let step = step % self.steps().max(1);
        self.hits.iter().filter(move |hit| hit.step == step)
    }
}

/// Look up a factory groove by id (`"keherwa"`, …).
pub fn groove_by_id(id: &str) -> Option<&'static TablaGroove> {
    GROOVES.iter().find(|g| g.id == id)
}

pub const DEFAULT_TABLA_GROOVE_ID: &str = "keherwa";

/// Keherwa theka: Dha Ge Na Ti | Na Ka Dhi Na (Ti → Ta).
pub const KEHERWA: TablaGroove = TablaGroove {
    id: "keherwa",
    name: "Keherwa",
    matras: 8,
    steps_per_matra: 1,
    hits: &[
        GrooveHit {
            step: 0,
            bol: Bol::Dha,
            amp: 1.0,
        },
        GrooveHit {
            step: 1,
            bol: Bol::Ge,
            amp: 0.75,
        },
        GrooveHit {
            step: 2,
            bol: Bol::Na,
            amp: 0.7,
        },
        GrooveHit {
            step: 3,
            bol: Bol::Ta,
            amp: 0.65,
        },
        GrooveHit {
            step: 4,
            bol: Bol::Na,
            amp: 0.7,
        },
        GrooveHit {
            step: 5,
            bol: Bol::Ka,
            amp: 0.7,
        },
        GrooveHit {
            step: 6,
            bol: Bol::Dhin,
            amp: 0.85,
        },
        GrooveHit {
            step: 7,
            bol: Bol::Na,
            amp: 0.65,
        },
    ],
};

/// Sparse trance pulse: Dha on sam, Na on khali.
pub const KEHERWA_PULSE: TablaGroove = TablaGroove {
    id: "keherwa_pulse",
    name: "Keherwa pulse",
    matras: 8,
    steps_per_matra: 1,
    hits: &[
        GrooveHit {
            step: 0,
            bol: Bol::Dha,
            amp: 1.0,
        },
        GrooveHit {
            step: 4,
            bol: Bol::Na,
            amp: 0.75,
        },
    ],
};

/// Dadra: Dha Dhin Na | Dha Tin Na (Tin → Tu).
pub const DADRA: TablaGroove = TablaGroove {
    id: "dadra",
    name: "Dadra",
    matras: 6,
    steps_per_matra: 1,
    hits: &[
        GrooveHit {
            step: 0,
            bol: Bol::Dha,
            amp: 1.0,
        },
        GrooveHit {
            step: 1,
            bol: Bol::Dhin,
            amp: 0.85,
        },
        GrooveHit {
            step: 2,
            bol: Bol::Na,
            amp: 0.7,
        },
        GrooveHit {
            step: 3,
            bol: Bol::Dha,
            amp: 0.9,
        },
        GrooveHit {
            step: 4,
            bol: Bol::Tu,
            amp: 0.75,
        },
        GrooveHit {
            step: 5,
            bol: Bol::Na,
            amp: 0.65,
        },
    ],
};

pub const GROOVES: &[TablaGroove] = &[KEHERWA, KEHERWA_PULSE, DADRA];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dha_expands_to_ge_and_na() {
        let parts = Bol::Dha.expand();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].0, TablaStroke::Ge);
        assert_eq!(parts[1].0, TablaStroke::Na);
    }

    #[test]
    fn dhin_expands_to_ge_and_tu() {
        let parts = Bol::Dhin.expand();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].0, TablaStroke::Ge);
        assert_eq!(parts[1].0, TablaStroke::Tu);
    }

    #[test]
    fn rest_expands_empty() {
        assert!(Bol::Rest.expand().is_empty());
    }

    #[test]
    fn keherwa_has_eight_matras_and_eight_hits() {
        assert_eq!(KEHERWA.matras, 8);
        assert_eq!(KEHERWA.steps(), 8);
        assert_eq!(KEHERWA.hits.len(), 8);
        assert_eq!(KEHERWA.hits_at(0).count(), 1);
        assert_eq!(KEHERWA.hits_at(0).next().unwrap().bol, Bol::Dha);
    }

    #[test]
    fn keherwa_pulse_is_sparse() {
        assert_eq!(KEHERWA_PULSE.hits.len(), 2);
        assert_eq!(KEHERWA_PULSE.hits_at(1).count(), 0);
        assert_eq!(KEHERWA_PULSE.hits_at(4).next().unwrap().bol, Bol::Na);
    }

    #[test]
    fn dadra_is_six() {
        assert_eq!(DADRA.matras, 6);
        assert_eq!(DADRA.hits.len(), 6);
    }

    #[test]
    fn catalog_lookup() {
        assert!(groove_by_id("keherwa").is_some());
        assert!(groove_by_id("keherwa_pulse").is_some());
        assert!(groove_by_id("dadra").is_some());
        assert!(groove_by_id("missing").is_none());
        assert_eq!(GROOVES.len(), 3);
    }
}
