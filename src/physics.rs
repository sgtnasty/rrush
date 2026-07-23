//! The quantum mechanics: hydrogen wave functions and the state currently on
//! screen. This module is deliberately free of any Bevy dependency — it is
//! pure math and is exercised directly by the unit tests at the bottom.

/// A selection of hydrogen wave functions, in atomic units (Bohr radius
/// a₀ = 1, ħ = 1). Left un-normalised — only the shape matters here.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Orbital {
    S1,
    S2,
    P2z,
    P3z,
    D3z2,
    D3xy,
}

impl Orbital {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Orbital::S1 => "1s",
            Orbital::S2 => "2s",
            Orbital::P2z => "2p_z",
            Orbital::P3z => "3p_z",
            Orbital::D3z2 => "3d_z2",
            Orbital::D3xy => "3d_xy",
        }
    }

    /// Principal quantum number n. Energy depends only on n.
    fn n(self) -> f32 {
        match self {
            Orbital::S1 => 1.0,
            Orbital::S2 | Orbital::P2z => 2.0,
            Orbital::P3z | Orbital::D3z2 | Orbital::D3xy => 3.0,
        }
    }

    /// Energy eigenvalue Eₙ = −1/(2n²) in Hartree.
    pub(crate) fn energy(self) -> f32 {
        let n = self.n();
        -0.5 / (n * n)
    }

    /// Normalisation constant so that ∫|ψ|²dV = 1 (a₀ = 1). Computed
    /// analytically from ∫r^n·e^(−ar)dr = n!/a^(n+1) and the angular
    /// integrals. This makes an equal-coefficient superposition an equal
    /// *probability* mixture, so the two states contribute symmetrically.
    fn norm(self) -> f32 {
        match self {
            Orbital::S1 => 0.564_190,
            Orbital::S2 | Orbital::P2z => 0.099_736,
            Orbital::P3z | Orbital::D3xy => 0.009_851,
            Orbital::D3z2 => 0.002_843,
        }
    }

    /// Signed, normalised amplitude ψ(x, y, z). The sign encodes the phase,
    /// which we use to colour the two lobes differently.
    pub(crate) fn psi(self, x: f32, y: f32, z: f32) -> f32 {
        let r = (x * x + y * y + z * z).sqrt();
        let shape = match self {
            Orbital::S1 => (-r).exp(),
            Orbital::S2 => (2.0 - r) * (-r / 2.0).exp(),
            Orbital::P2z => z * (-r / 2.0).exp(),
            Orbital::P3z => z * (6.0 - r) * (-r / 3.0).exp(),
            Orbital::D3z2 => (3.0 * z * z - r * r) * (-r / 3.0).exp(),
            Orbital::D3xy => x * y * (-r / 3.0).exp(),
        };
        self.norm() * shape
    }

    /// Half-width of the cube (in a₀) that comfortably contains the cloud.
    pub(crate) fn view_radius(self) -> f32 {
        match self {
            Orbital::S1 => 4.5,
            Orbital::S2 | Orbital::P2z => 13.0,
            Orbital::P3z | Orbital::D3z2 | Orbital::D3xy => 26.0,
        }
    }
}

/// One weighted term cₖ·ψₖ of a superposition.
#[derive(Clone, Copy)]
pub(crate) struct Term {
    pub(crate) orbital: Orbital,
    pub(crate) coeff: f32,
}

/// The thing currently on screen: either a lone eigenstate or a
/// time-evolving superposition.
///
/// NOTE: every superposition preset must combine orbitals with *distinct*
/// principal numbers n. Only then do the cross terms average to zero, so the
/// sampling envelope Σ cₖ²·ψₖ² is exact and the state actually evolves in
/// time (ΔE ≠ 0). A same-n combination would be stationary *and* would make
/// the envelope miss the cross-term support.
#[derive(Clone)]
pub(crate) enum SimState {
    Single(Orbital),
    Superposition {
        name: &'static str,
        terms: Vec<Term>,
    },
}

impl SimState {
    /// Build the normalised superposition for preset keys 7..0.
    pub(crate) fn preset(index: usize) -> SimState {
        let (name, raw): (&str, &[(Orbital, f32)]) = match index {
            0 => ("1s + 2p_z", &[(Orbital::S1, 1.0), (Orbital::P2z, 1.0)]),
            1 => ("1s + 2s", &[(Orbital::S1, 1.0), (Orbital::S2, 1.0)]),
            2 => ("2p_z + 3d_z2", &[(Orbital::P2z, 1.0), (Orbital::D3z2, 1.0)]),
            _ => (
                "1s + 2p_z + 3d_z2",
                &[(Orbital::S1, 1.0), (Orbital::P2z, 1.0), (Orbital::D3z2, 1.0)],
            ),
        };
        let norm = raw.iter().map(|(_, c)| c * c).sum::<f32>().sqrt();
        let terms = raw
            .iter()
            .map(|&(orbital, c)| Term {
                orbital,
                coeff: c / norm,
            })
            .collect();
        SimState::Superposition { name, terms }
    }

    pub(crate) fn name(&self) -> &str {
        match self {
            SimState::Single(o) => o.name(),
            SimState::Superposition { name, .. } => name,
        }
    }

    pub(crate) fn is_dynamic(&self) -> bool {
        matches!(self, SimState::Superposition { .. })
    }

    pub(crate) fn terms(&self) -> Vec<Term> {
        match self {
            SimState::Single(o) => vec![Term {
                orbital: *o,
                coeff: 1.0,
            }],
            SimState::Superposition { terms, .. } => terms.clone(),
        }
    }

    pub(crate) fn view_radius(&self) -> f32 {
        self.terms()
            .iter()
            .map(|t| t.orbital.view_radius())
            .fold(0.0, f32::max)
    }

    /// Time-averaged density Σ cₖ²·ψₖ², used to place the fixed sample points.
    pub(crate) fn envelope(&self, x: f32, y: f32, z: f32) -> f32 {
        self.terms()
            .iter()
            .map(|t| {
                let p = t.coeff * t.orbital.psi(x, y, z);
                p * p
            })
            .sum()
    }

    /// Instantaneous complex amplitude Ψ(r,t) = Σ cₖ·ψₖ·e^(−iEₖt),
    /// returned as (real, imag).
    pub(crate) fn amplitude(&self, x: f32, y: f32, z: f32, t: f32) -> (f32, f32) {
        let mut re = 0.0;
        let mut im = 0.0;
        for term in self.terms() {
            let a = term.coeff * term.orbital.psi(x, y, z);
            let phase = term.orbital.energy() * t;
            re += a * phase.cos();
            im -= a * phase.sin();
        }
        (re, im)
    }

    /// A default animation speed so the dominant Bohr frequency is watchable.
    pub(crate) fn suggested_speed(&self) -> f32 {
        let terms = self.terms();
        let mut max_dw = 0.0f32;
        for (i, a) in terms.iter().enumerate() {
            for b in &terms[i + 1..] {
                max_dw = max_dw.max((a.orbital.energy() - b.orbital.energy()).abs());
            }
        }
        if max_dw <= f32::EPSILON {
            4.0
        } else {
            (1.5 / max_dw).clamp(1.0, 12.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Numerically integrate the density's centre of charge ⟨z⟩ at time `t`.
    fn mean_z(state: &SimState, t: f32) -> f32 {
        let reach = state.view_radius();
        let steps = 60;
        let step = 2.0 * reach / steps as f32;
        let mut num = 0.0f64;
        let mut den = 0.0f64;
        for i in 0..steps {
            let x = -reach + (i as f32 + 0.5) * step;
            for j in 0..steps {
                let y = -reach + (j as f32 + 0.5) * step;
                for k in 0..steps {
                    let z = -reach + (k as f32 + 0.5) * step;
                    let (re, im) = state.amplitude(x, y, z, t);
                    let d = (re * re + im * im) as f64;
                    num += z as f64 * d;
                    den += d;
                }
            }
        }
        (num / den) as f32
    }

    /// The 1s + 2p_z dipole must slosh along z: ⟨z⟩ oscillates, is ~0 at a
    /// quarter period, and flips sign at the half period. This is the check
    /// that the *instantaneous* |Ψ(r,t)|² is rendered, not the time average.
    #[test]
    fn dipole_oscillates_along_z() {
        let state = SimState::preset(0); // 1s + 2p_z
        let dw = (Orbital::S1.energy() - Orbital::P2z.energy()).abs(); // 0.375
        let period = std::f32::consts::TAU / dw;

        let z0 = mean_z(&state, 0.0);
        let z_quarter = mean_z(&state, period * 0.25);
        let z_half = mean_z(&state, period * 0.5);

        // Real displacement at the extremes — near the known transition
        // dipole ⟨1s|z|2p_z⟩ = 128√2/243 ≈ 0.745 a₀.
        assert!(z0.abs() > 0.5, "expected a displaced charge cloud, got {z0}");
        // Opposite side half a period later.
        assert!(
            z0 * z_half < 0.0 && (z0 + z_half).abs() < 0.1 * z0.abs(),
            "expected ⟨z⟩ to flip sign: z0={z0}, z_half={z_half}"
        );
        // Balanced at the quarter period.
        assert!(
            z_quarter.abs() < 0.1 * z0.abs(),
            "expected ⟨z⟩≈0 at quarter period, got {z_quarter}"
        );
    }

    /// The time-averaged envelope must be centred (it is symmetric in z), so a
    /// static rendering would sit at the origin — proving the animation, not
    /// the envelope, is what produces the motion above.
    #[test]
    fn envelope_is_centred() {
        let state = SimState::preset(0);
        let reach = state.view_radius();
        let steps = 60;
        let step = 2.0 * reach / steps as f32;
        let mut num = 0.0f64;
        let mut den = 0.0f64;
        for i in 0..steps {
            let x = -reach + (i as f32 + 0.5) * step;
            for j in 0..steps {
                let y = -reach + (j as f32 + 0.5) * step;
                for k in 0..steps {
                    let z = -reach + (k as f32 + 0.5) * step;
                    let d = state.envelope(x, y, z) as f64;
                    num += z as f64 * d;
                    den += d;
                }
            }
        }
        assert!((num / den).abs() < 0.05, "envelope ⟨z⟩ should be ~0");
    }
}
