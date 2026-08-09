//! Simulator-only rendezvous-energy model for the MMP/Φ research track.
//!
//! This is deliberately not a protocol parameter or a radio driver. It models
//! only the controlled hypothesis `hazard ≈ q · λ · ρ`, where `λ` and `ρ` are
//! normalized transmitter and receiver activity rates. The caller must replace
//! the idealized result with measured controller, OS, and whole-device energy.

/// One normalized rendezvous activity point and its modeled energy split.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RendezvousActivity {
    /// Normalized advertisement/transmit activity per day.
    pub tx_activity: f64,
    /// Normalized scan/receive activity per day.
    pub rx_activity: f64,
    /// Modeled transmit energy in the caller's daily energy unit.
    pub tx_energy: f64,
    /// Modeled receive energy in the caller's daily energy unit.
    pub rx_energy: f64,
    /// Sum of [`Self::tx_energy`] and [`Self::rx_energy`].
    pub energy: f64,
}

/// Controlled square-root optimum for the energy-dual rendezvous hypothesis.
///
/// Activity values are normalized rates rather than BLE timings. The model is
/// useful for comparing candidate duty-cycle points, but it does not account
/// for collisions, controller scheduling, OS wakeups, or platform support.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RendezvousEnergyModel {
    baseline_tx_activity: f64,
    baseline_rx_activity: f64,
    tx_cost_per_activity: f64,
    rx_cost_per_activity: f64,
    hazard_product: f64,
}

impl RendezvousEnergyModel {
    /// Construct a model from one measured baseline point.
    ///
    /// All four arguments must be finite and strictly positive. The baseline
    /// rendezvous product is retained as the fixed-hazard target for the
    /// idealized optimization.
    pub fn from_baseline(
        tx_activity: f64,
        rx_activity: f64,
        tx_cost_per_activity: f64,
        rx_cost_per_activity: f64,
    ) -> Option<Self> {
        if [tx_activity, rx_activity, tx_cost_per_activity, rx_cost_per_activity]
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
        {
            return None;
        }
        let hazard_product = tx_activity * rx_activity;
        let tx_energy = tx_activity * tx_cost_per_activity;
        let rx_energy = rx_activity * rx_cost_per_activity;
        let optimum_tx_activity = hazard_product * rx_cost_per_activity / tx_cost_per_activity;
        let optimum_rx_activity = hazard_product * tx_cost_per_activity / rx_cost_per_activity;
        let optimum_tx_energy = optimum_tx_activity.sqrt() * tx_cost_per_activity;
        let optimum_rx_energy = optimum_rx_activity.sqrt() * rx_cost_per_activity;
        if [
            hazard_product,
            tx_energy,
            rx_energy,
            tx_energy + rx_energy,
            optimum_tx_activity,
            optimum_rx_activity,
            optimum_tx_energy,
            optimum_rx_energy,
            optimum_tx_energy + optimum_rx_energy,
        ]
        .iter()
        .any(|value| !value.is_finite() || *value <= 0.0)
        {
            return None;
        }
        Some(Self {
            baseline_tx_activity: tx_activity,
            baseline_rx_activity: rx_activity,
            tx_cost_per_activity,
            rx_cost_per_activity,
            hazard_product,
        })
    }

    /// The baseline modeled energy in the caller's daily energy unit.
    pub fn baseline_energy(&self) -> f64 {
        self.baseline_tx_activity * self.tx_cost_per_activity
            + self.baseline_rx_activity * self.rx_cost_per_activity
    }

    /// The fixed normalized hazard product retained by the model.
    pub fn hazard_product(&self) -> f64 {
        self.hazard_product
    }

    /// Evaluate one candidate activity point under the fixed-hazard model.
    pub fn evaluate(&self, tx_activity: f64, rx_activity: f64) -> Option<RendezvousActivity> {
        if !tx_activity.is_finite()
            || !rx_activity.is_finite()
            || tx_activity <= 0.0
            || rx_activity <= 0.0
            || (tx_activity * rx_activity - self.hazard_product).abs() > self.hazard_product * 1e-9
        {
            return None;
        }
        let tx_energy = tx_activity * self.tx_cost_per_activity;
        let rx_energy = rx_activity * self.rx_cost_per_activity;
        Some(RendezvousActivity {
            tx_activity,
            rx_activity,
            tx_energy,
            rx_energy,
            energy: tx_energy + rx_energy,
        })
    }

    /// Compute the square-root minimum of `c_tx·λ + c_rx·ρ` subject to
    /// `λ·ρ = hazard_product`.
    pub fn optimum(&self) -> RendezvousActivity {
        let tx_activity = (self.hazard_product * self.rx_cost_per_activity / self.tx_cost_per_activity).sqrt();
        let rx_activity = (self.hazard_product * self.tx_cost_per_activity / self.rx_cost_per_activity).sqrt();
        // The constructor guarantees positive finite inputs, so this cannot
        // fail unless a future caller changes the representation invariants.
        self.evaluate(tx_activity, rx_activity).expect("valid square-root optimum")
    }

    /// Baseline energy divided by the idealized optimum energy.
    pub fn idealized_gain(&self) -> f64 {
        self.baseline_energy() / self.optimum().energy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edr_optimum_balances_energy_contributions_and_preserves_hazard() {
        let model = RendezvousEnergyModel::from_baseline(1.0, 1.0, 0.12, 4.5).unwrap();
        let optimum = model.optimum();

        assert!((optimum.tx_activity * optimum.rx_activity - model.hazard_product()).abs() < 1e-12);
        assert!((optimum.tx_energy - optimum.rx_energy).abs() < 1e-12);
        assert!(optimum.energy < model.baseline_energy());
    }

    #[test]
    fn edr_reports_the_idealized_gain_without_claiming_a_radio_result() {
        let model = RendezvousEnergyModel::from_baseline(1.0, 1.0, 0.12, 4.5).unwrap();
        let gain = model.idealized_gain();

        assert!(gain > 3.0 && gain < 3.3);
    }

    #[test]
    fn edr_rejects_nonrepresentable_baseline_terms() {
        assert!(RendezvousEnergyModel::from_baseline(f64::MAX, f64::MAX, 1.0, 1.0).is_none());
        assert!(RendezvousEnergyModel::from_baseline(
            f64::MIN_POSITIVE,
            f64::MIN_POSITIVE,
            1.0,
            1.0,
        )
        .is_none());
        assert!(RendezvousEnergyModel::from_baseline(1.0, 1.0, f64::MAX, f64::MAX).is_none());
    }
}
