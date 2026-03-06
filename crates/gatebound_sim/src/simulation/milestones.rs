use gatebound_domain::*;

use super::state::Simulation;

impl Simulation {
    pub(in crate::simulation) fn update_milestones(&mut self) {
        let throughput_current = self
            .gate_throughput_view()
            .into_iter()
            .map(|snapshot| snapshot.player_share)
            .fold(0.0_f64, f64::max);
        let market_share_current = self.market_share_view();

        for milestone in &mut self.milestones {
            milestone.current = match milestone.id {
                MilestoneId::Capital => self.capital,
                MilestoneId::MarketShare => market_share_current,
                MilestoneId::ThroughputControl => throughput_current,
                MilestoneId::Reputation => self.reputation,
            };
            if !milestone.completed && milestone.current >= milestone.target {
                milestone.completed = true;
                milestone.completed_cycle = Some(self.cycle);
            }
        }
    }
}
