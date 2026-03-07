use std::collections::{BTreeMap, VecDeque};

use gatebound_domain::*;

#[derive(Debug, Clone)]
pub struct Simulation {
    pub(crate) config: RuntimeConfig,
    pub(crate) world: World,
    pub(crate) tick: u64,
    pub(crate) cycle: u64,
    pub(crate) companies: BTreeMap<CompanyId, Company>,
    pub(crate) npc_company_runtimes: BTreeMap<CompanyId, NpcCompanyRuntime>,
    pub(crate) markets: BTreeMap<StationId, MarketBook>,
    pub(crate) player_station_storage: BTreeMap<StationId, BTreeMap<Commodity, f64>>,
    pub(crate) player_mission_storage: BTreeMap<StationId, BTreeMap<MissionId, f64>>,
    pub(crate) missions: BTreeMap<MissionId, Mission>,
    pub(crate) mission_offers: BTreeMap<u64, MissionOffer>,
    pub(crate) next_mission_offer_id: u64,
    pub(crate) trade_orders: BTreeMap<TradeOrderId, TradeOrder>,
    pub(crate) next_trade_order_id: u64,
    pub(crate) ships: BTreeMap<ShipId, Ship>,
    pub(crate) milestones: Vec<MilestoneStatus>,
    pub(crate) capital: f64,
    pub(crate) active_loan: Option<ActiveLoan>,
    pub(crate) outstanding_debt: f64,
    pub(crate) reputation: f64,
    pub(crate) current_loan_interest_rate: f64,
    pub(crate) gate_traversals_cycle: BTreeMap<GateId, BTreeMap<CompanyId, u32>>,
    pub(crate) gate_traversals_window: VecDeque<BTreeMap<GateId, BTreeMap<CompanyId, u32>>>,
    pub(crate) queue_delay_accumulator: u64,
    pub(crate) reroute_count: u64,
    pub(crate) sla_successes: u64,
    pub(crate) sla_failures: u64,
    pub(crate) gate_queue_load: BTreeMap<GateId, f64>,
    pub(crate) ship_idle_ticks_cycle: BTreeMap<ShipId, u32>,
    pub(crate) ship_delay_ticks_cycle: BTreeMap<ShipId, u32>,
    pub(crate) ship_runs_completed: BTreeMap<ShipId, u32>,
    pub(crate) ship_profit_earned: BTreeMap<ShipId, f64>,
    pub(crate) previous_cycle_prices: BTreeMap<(StationId, Commodity), f64>,
    pub(crate) modifiers: Vec<ActiveModifier>,
}

#[derive(Debug, Clone)]
pub(crate) struct ActiveModifier {
    pub(crate) until_tick: u64,
    pub(crate) gate: Option<GateId>,
    pub(crate) risk: RiskStageA,
    pub(crate) magnitude: f64,
}
