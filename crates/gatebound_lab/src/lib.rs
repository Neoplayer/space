use std::fs;
use std::path::{Path, PathBuf};

use gatebound_domain::{Commodity, RuntimeConfig};
use gatebound_sim::{EconomyLabSnapshot, PlannerMode, PlannerSettings, Simulation};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq)]
pub struct LabRunSpec {
    pub systems: Vec<u8>,
    pub ticks: u64,
    pub seeds: usize,
    pub planner_mode: PlannerMode,
    pub output_dir: PathBuf,
    pub npc_ship_count: Option<usize>,
    pub station_count_min: Option<u8>,
    pub station_count_max: Option<u8>,
    pub planning_interval_ticks: Option<u64>,
    pub critical_shortage_threshold: Option<f64>,
    pub dispatch_window_ticks: Option<u64>,
    pub minimum_load_factor: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct LabRunSummary {
    system_count: u8,
    seed: u64,
    snapshot: EconomyLabSnapshot,
    average_zero_stock_ratio: f64,
    average_convoy_index: f64,
    peak_critical_shortage_ratio: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct LabSummaryEnvelope {
    planner_mode: PlannerMode,
    ticks: u64,
    seeds: usize,
    systems: Vec<u8>,
    runs: Vec<LabRunSummary>,
}

pub fn parse_args<I, S>(args: I) -> Result<LabRunSpec, String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut args = args.into_iter().map(Into::into).collect::<Vec<_>>();
    if args.is_empty() {
        return Err("expected argv".to_string());
    }
    args.remove(0);
    if args.first().map(String::as_str) != Some("run") {
        return Err("expected `run` subcommand".to_string());
    }
    args.remove(0);

    let mut systems = None;
    let mut ticks = None;
    let mut seeds = None;
    let mut planner_mode = None;
    let mut output_dir = None;
    let mut npc_ship_count = None;
    let mut station_count_min = None;
    let mut station_count_max = None;
    let mut planning_interval_ticks = None;
    let mut critical_shortage_threshold = None;
    let mut dispatch_window_ticks = None;
    let mut minimum_load_factor = None;

    let mut idx = 0;
    while idx < args.len() {
        let key = args[idx].as_str();
        let value = args
            .get(idx + 1)
            .ok_or_else(|| format!("missing value for `{key}`"))?;
        match key {
            "--planner" => planner_mode = Some(parse_planner_mode(value)?),
            "--systems" => systems = Some(parse_systems(value)?),
            "--ticks" => {
                ticks = Some(
                    value
                        .parse::<u64>()
                        .map_err(|e| format!("invalid ticks: {e}"))?,
                )
            }
            "--seeds" => {
                seeds = Some(
                    value
                        .parse::<usize>()
                        .map_err(|e| format!("invalid seeds: {e}"))?,
                )
            }
            "--output-dir" => output_dir = Some(PathBuf::from(value)),
            "--npc-ships" => {
                npc_ship_count = Some(
                    value
                        .parse::<usize>()
                        .map_err(|e| format!("invalid npc ship count: {e}"))?,
                )
            }
            "--station-range" => {
                let (min, max) = parse_station_range(value)?;
                station_count_min = Some(min);
                station_count_max = Some(max);
            }
            "--planning-interval" => {
                planning_interval_ticks = Some(
                    value
                        .parse::<u64>()
                        .map_err(|e| format!("invalid planning interval: {e}"))?,
                )
            }
            "--critical-threshold" => {
                critical_shortage_threshold = Some(
                    value
                        .parse::<f64>()
                        .map_err(|e| format!("invalid critical threshold: {e}"))?,
                )
            }
            "--dispatch-window" => {
                dispatch_window_ticks = Some(
                    value
                        .parse::<u64>()
                        .map_err(|e| format!("invalid dispatch window: {e}"))?,
                )
            }
            "--min-load-factor" => {
                minimum_load_factor = Some(
                    value
                        .parse::<f64>()
                        .map_err(|e| format!("invalid min load factor: {e}"))?,
                )
            }
            _ => return Err(format!("unknown argument `{key}`")),
        }
        idx += 2;
    }

    Ok(LabRunSpec {
        systems: systems.ok_or_else(|| "missing --systems".to_string())?,
        ticks: ticks.ok_or_else(|| "missing --ticks".to_string())?,
        seeds: seeds.ok_or_else(|| "missing --seeds".to_string())?,
        planner_mode: planner_mode.ok_or_else(|| "missing --planner".to_string())?,
        output_dir: output_dir.ok_or_else(|| "missing --output-dir".to_string())?,
        npc_ship_count,
        station_count_min,
        station_count_max,
        planning_interval_ticks,
        critical_shortage_threshold,
        dispatch_window_ticks,
        minimum_load_factor,
    })
}

pub fn run_lab(spec: &LabRunSpec) -> Result<(), String> {
    fs::create_dir_all(&spec.output_dir)
        .map_err(|e| format!("failed to create output dir: {e}"))?;

    let mut timeseries_rows = vec![timeseries_header()];
    let mut station_rows = vec![station_header()];
    let mut lane_rows = vec![lane_header()];
    let mut runs = Vec::new();

    for system_count in &spec.systems {
        for seed in 1..=spec.seeds {
            let seed = seed as u64;
            let mut sim = Simulation::new(lab_runtime_config(*system_count, spec)?, seed);
            sim.set_planner_mode(spec.planner_mode);
            sim.set_planner_settings(lab_planner_settings(spec));
            if let Some(npc_ship_count) = spec.npc_ship_count {
                sim.set_npc_trade_ship_count(npc_ship_count);
            }

            let mut zero_stock_sum = 0.0;
            let mut convoy_sum = 0.0;
            let mut peak_critical_shortage_ratio = 0.0_f64;
            let mut snapshots = 0_u64;
            record_timeseries_row(
                &mut timeseries_rows,
                *system_count,
                seed,
                &sim.economy_lab_snapshot(),
            );

            let cycle_ticks = sim.time_settings_view().cycle_ticks.max(1);
            for _ in 0..spec.ticks {
                sim.step_tick();
                if sim.tick().is_multiple_of(u64::from(cycle_ticks)) || sim.tick() == spec.ticks {
                    let snapshot = sim.economy_lab_snapshot();
                    zero_stock_sum += snapshot.zero_stock_ratio;
                    convoy_sum += snapshot.convoy_index;
                    peak_critical_shortage_ratio =
                        peak_critical_shortage_ratio.max(snapshot.critical_shortage_ratio);
                    snapshots = snapshots.saturating_add(1);
                    record_timeseries_row(&mut timeseries_rows, *system_count, seed, &snapshot);
                }
            }

            record_station_rows(&mut station_rows, *system_count, seed, &sim);
            record_lane_rows(&mut lane_rows, *system_count, seed, &sim);

            let final_snapshot = sim.economy_lab_snapshot();
            runs.push(LabRunSummary {
                system_count: *system_count,
                seed,
                snapshot: final_snapshot,
                average_zero_stock_ratio: if snapshots == 0 {
                    0.0
                } else {
                    zero_stock_sum / snapshots as f64
                },
                average_convoy_index: if snapshots == 0 {
                    0.0
                } else {
                    convoy_sum / snapshots as f64
                },
                peak_critical_shortage_ratio,
            });
        }
    }

    let summary = LabSummaryEnvelope {
        planner_mode: spec.planner_mode,
        ticks: spec.ticks,
        seeds: spec.seeds,
        systems: spec.systems.clone(),
        runs,
    };

    write_file(
        &spec.output_dir.join("summary.json"),
        &serde_json::to_string_pretty(&summary)
            .map_err(|e| format!("failed to serialize summary: {e}"))?,
    )?;
    write_file(
        &spec.output_dir.join("timeseries.csv"),
        &timeseries_rows.join("\n"),
    )?;
    write_file(
        &spec.output_dir.join("station_snapshot.csv"),
        &station_rows.join("\n"),
    )?;
    write_file(
        &spec.output_dir.join("lane_snapshot.csv"),
        &lane_rows.join("\n"),
    )?;

    Ok(())
}

fn write_file(path: &Path, contents: &str) -> Result<(), String> {
    fs::write(path, contents).map_err(|e| format!("failed to write {}: {e}", path.display()))
}

fn parse_planner_mode(value: &str) -> Result<PlannerMode, String> {
    match value {
        "greedy" | "current" => Ok(PlannerMode::GreedyCurrent),
        "global" => Ok(PlannerMode::GlobalOnly),
        "hybrid" => Ok(PlannerMode::HybridRecommended),
        _ => Err(format!("unknown planner mode `{value}`")),
    }
}

fn parse_systems(value: &str) -> Result<Vec<u8>, String> {
    let mut systems = value
        .split(',')
        .map(|chunk| chunk.trim())
        .filter(|chunk| !chunk.is_empty())
        .map(|chunk| {
            chunk
                .parse::<u8>()
                .map_err(|e| format!("invalid system count `{chunk}`: {e}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if systems.is_empty() {
        return Err("at least one system count is required".to_string());
    }
    systems.sort_unstable();
    systems.dedup();
    Ok(systems)
}

fn parse_station_range(value: &str) -> Result<(u8, u8), String> {
    let mut parts = value.split(':');
    let min = parts
        .next()
        .ok_or_else(|| "station range must contain min:max".to_string())?
        .parse::<u8>()
        .map_err(|e| format!("invalid station range min: {e}"))?;
    let max = parts
        .next()
        .ok_or_else(|| "station range must contain min:max".to_string())?
        .parse::<u8>()
        .map_err(|e| format!("invalid station range max: {e}"))?;
    Ok((min, max))
}

fn lab_runtime_config(system_count: u8, spec: &LabRunSpec) -> Result<RuntimeConfig, String> {
    let mut config = RuntimeConfig::default();
    config.galaxy.system_count = system_count;
    if system_count < config.galaxy.cluster_size_min {
        config.galaxy.cluster_size_min = system_count.max(1);
    }
    if system_count < config.galaxy.cluster_size_max {
        config.galaxy.cluster_size_max = system_count.max(config.galaxy.cluster_size_min);
    }
    if let Some(min) = spec.station_count_min {
        config.galaxy.station_count_min = min;
    }
    if let Some(max) = spec.station_count_max {
        config.galaxy.station_count_max = max;
    }
    config
        .validate()
        .map_err(|e| format!("invalid lab runtime config: {e}"))?;
    Ok(config)
}

fn lab_planner_settings(spec: &LabRunSpec) -> PlannerSettings {
    let mut settings = PlannerSettings::default();
    if let Some(value) = spec.planning_interval_ticks {
        settings.planning_interval_ticks = value.max(1);
    }
    if let Some(value) = spec.critical_shortage_threshold {
        settings.emergency_stock_coverage = value.clamp(0.0, 1.0);
    }
    if let Some(value) = spec.dispatch_window_ticks {
        settings.dispatch_window_ticks = value.max(1);
    }
    if let Some(value) = spec.minimum_load_factor {
        settings.minimum_load_factor = value.clamp(0.0, 1.0);
    }
    settings
}

fn timeseries_header() -> String {
    "system_count,seed,tick,cycle,avg_price_index,aggregate_stock_coverage,zero_stock_ratio,critical_shortage_ratio,critical_shortage_count,order_fill_ratio,avg_ship_load_factor,npc_idle_ratio,convoy_index,lane_concentration,p95_gate_load,avg_price_spread_pct,unmatched_critical_demands,active_trade_orders,total_reserved_amount".to_string()
}

fn station_header() -> String {
    "system_count,seed,system_id,station_id,price_index,stock_coverage,shortage_count,surplus_count,strongest_shortage,strongest_surplus,best_buy,best_sell".to_string()
}

fn lane_header() -> String {
    "system_count,seed,order_id,source_station,destination_station,commodity,total_amount,reserved_amount,remaining_amount,urgency_score,is_critical,assigned_ships,lane_ship_cap".to_string()
}

fn record_timeseries_row(
    rows: &mut Vec<String>,
    system_count: u8,
    seed: u64,
    snapshot: &EconomyLabSnapshot,
) {
    rows.push(format!(
        "{system_count},{seed},{},{},{:.6},{:.6},{:.6},{:.6},{},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{},{},{}",
        snapshot.tick,
        snapshot.cycle,
        snapshot.avg_price_index,
        snapshot.aggregate_stock_coverage,
        snapshot.zero_stock_ratio,
        snapshot.critical_shortage_ratio,
        snapshot.critical_shortage_count,
        snapshot.order_fill_ratio,
        snapshot.avg_ship_load_factor,
        snapshot.npc_idle_ratio,
        snapshot.convoy_index,
        snapshot.lane_concentration,
        snapshot.p95_gate_load,
        snapshot.avg_price_spread_pct,
        snapshot.unmatched_critical_demands,
        snapshot.active_trade_orders,
        snapshot.total_reserved_amount,
    ));
}

fn record_station_rows(rows: &mut Vec<String>, system_count: u8, seed: u64, sim: &Simulation) {
    let topology = sim.camera_topology_view();
    for system in topology.systems {
        for station in system.stations {
            let detail = sim
                .market_panel_view(system.system_id, Some(station.station_id), Commodity::Fuel)
                .station_detail;
            let Some(detail) = detail else {
                continue;
            };
            rows.push(format!(
                "{system_count},{seed},{},{},{:.6},{:.6},{},{},{:?},{:?},{:?},{:?}",
                system.system_id.0,
                station.station_id.0,
                detail.price_index,
                detail.stock_coverage,
                detail.shortage_count,
                detail.surplus_count,
                detail.strongest_shortage_commodity,
                detail.strongest_surplus_commodity,
                detail.best_buy_commodity,
                detail.best_sell_commodity,
            ));
        }
    }
}

fn record_lane_rows(rows: &mut Vec<String>, system_count: u8, seed: u64, sim: &Simulation) {
    let diagnostics = sim.planner_diagnostics();
    for order in diagnostics.orders {
        rows.push(format!(
            "{system_count},{seed},{},{},{},{:?},{:.6},{:.6},{:.6},{:.6},{},{},{}",
            order.order_id,
            order.source_station.0,
            order.destination_station.0,
            order.commodity,
            order.total_amount,
            order.reserved_amount,
            order.remaining_amount,
            order.urgency_score,
            order.is_critical,
            order.assigned_ships,
            order.lane_ship_cap,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_args_accepts_comma_separated_systems_and_hybrid_alias() {
        let spec = parse_args([
            "gatebound_lab",
            "run",
            "--planner",
            "hybrid",
            "--systems",
            "5,10,25",
            "--ticks",
            "200",
            "--seeds",
            "2",
            "--output-dir",
            "/tmp/gatebound-lab-parse",
        ])
        .expect("cli parsing should succeed");

        assert_eq!(spec.planner_mode, PlannerMode::HybridRecommended);
        assert_eq!(spec.systems, vec![5, 10, 25]);
        assert_eq!(spec.ticks, 200);
        assert_eq!(spec.seeds, 2);
    }

    #[test]
    fn run_lab_writes_summary_and_csv_artifacts() {
        let output_dir = std::env::temp_dir().join("gatebound_lab_outputs");
        if output_dir.exists() {
            std::fs::remove_dir_all(&output_dir).expect("old lab output dir should clear");
        }
        std::fs::create_dir_all(&output_dir).expect("output dir should create");

        let spec = LabRunSpec {
            systems: vec![5, 10],
            ticks: 120,
            seeds: 2,
            planner_mode: PlannerMode::HybridRecommended,
            output_dir: output_dir.clone(),
            npc_ship_count: Some(12),
            station_count_min: None,
            station_count_max: None,
            planning_interval_ticks: Some(6),
            critical_shortage_threshold: Some(0.10),
            dispatch_window_ticks: Some(12),
            minimum_load_factor: Some(0.60),
        };

        run_lab(&spec).expect("lab run should succeed");

        for file_name in [
            "summary.json",
            "timeseries.csv",
            "station_snapshot.csv",
            "lane_snapshot.csv",
        ] {
            assert!(
                output_dir.join(file_name).exists(),
                "{file_name} should be written"
            );
        }
    }
}
