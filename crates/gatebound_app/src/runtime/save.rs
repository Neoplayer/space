use bevy::prelude::{ButtonInput, KeyCode, Res, ResMut, Resource};
use gatebound_domain::RuntimeConfig;
use gatebound_sim::Simulation;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(not(target_arch = "wasm32"))]
use std::fs;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

use crate::features::finance::FinanceUiState;
use crate::features::markets::MarketsUiState;
use crate::features::missions::MissionsPanelState;
use crate::features::ships::ShipUiState;
use crate::features::stations::StationUiState;
use crate::input::camera::CameraUiState;
use crate::runtime::sim::{
    SelectedShip, SelectedStation, SelectedSystem, SimClock, SimResource, TrackedShip,
    UiKpiTracker, UiPanelState,
};
use crate::ui::hud::HudMessages;

const STORAGE_MANIFEST_KEY: &str = "gatebound.saves.manifest.v1";
const STORAGE_PAYLOAD_PREFIX: &str = "gatebound.saves.payload.v1.";
const DESKTOP_MANIFEST_FILE: &str = "manifest.v1.json";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameSaveSummary {
    pub id: String,
    pub display_name: String,
    pub saved_at_unix: u64,
    pub world_time_label: String,
    pub capital: f64,
    pub debt: f64,
    pub reputation: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GameSaveEnvelope {
    pub summary: GameSaveSummary,
    pub payload: String,
}

impl GameSaveEnvelope {
    pub fn capture_new(simulation: &Simulation) -> Result<Self, SaveStorageError> {
        let saved_at_unix = current_unix_seconds();
        Self::capture(
            format!("save-{}", current_unix_nanos()),
            auto_save_name_from_timestamp(saved_at_unix),
            saved_at_unix,
            simulation,
        )
    }

    pub fn capture_overwrite(
        existing: &GameSaveSummary,
        simulation: &Simulation,
    ) -> Result<Self, SaveStorageError> {
        Self::capture(
            existing.id.clone(),
            existing.display_name.clone(),
            current_unix_seconds(),
            simulation,
        )
    }

    pub fn into_simulation(self, config: RuntimeConfig) -> Result<Simulation, SaveStorageError> {
        Simulation::from_snapshot_payload(&self.payload, config)
            .map_err(|error| SaveStorageError::Parse(error.to_string()))
    }

    fn capture(
        id: String,
        display_name: String,
        saved_at_unix: u64,
        simulation: &Simulation,
    ) -> Result<Self, SaveStorageError> {
        let payload = simulation
            .snapshot_payload()
            .map_err(|error| SaveStorageError::Parse(error.to_string()))?;
        Ok(Self {
            summary: GameSaveSummary {
                id,
                display_name,
                saved_at_unix,
                world_time_label: format_world_time_label(
                    simulation.tick(),
                    simulation.time_settings_view(),
                ),
                capital: simulation.capital(),
                debt: simulation.outstanding_debt(),
                reputation: simulation.reputation(),
            },
            payload,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingSaveAction {
    Load(String),
    Overwrite(String),
}

#[derive(Resource, Debug, Clone, PartialEq, Default)]
pub struct SaveMenuState {
    pub open: bool,
    pub selected_entry_id: Option<String>,
    pub entries: Vec<GameSaveSummary>,
    pub pending_action: Option<PendingSaveAction>,
    pub last_error: Option<String>,
    pub paused_before_open: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveStorageError {
    Io(String),
    Parse(String),
    NotFound(String),
    Unavailable(String),
}

impl Display for SaveStorageError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(message)
            | Self::Parse(message)
            | Self::NotFound(message)
            | Self::Unavailable(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for SaveStorageError {}

#[derive(Resource, Debug, Clone)]
pub struct SaveStorage {
    backend: SaveStorageBackend,
}

#[derive(Debug, Clone)]
enum SaveStorageBackend {
    #[cfg(not(target_arch = "wasm32"))]
    Desktop(DesktopSaveStorage),
    #[cfg(target_arch = "wasm32")]
    Web,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Default)]
struct DesktopSaveStorage {
    override_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct StorageManifest {
    entries: Vec<StoredSaveMetadata>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct StoredSaveMetadata {
    id: String,
    display_name: String,
    saved_at_unix: u64,
    world_time_label: String,
    capital: f64,
    debt: f64,
    reputation: f64,
}

impl SaveStorage {
    pub fn new() -> Self {
        Self {
            backend: SaveStorageBackend::new(),
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test_desktop_dir(save_dir: PathBuf) -> Self {
        Self {
            backend: SaveStorageBackend::for_test_desktop_dir(save_dir),
        }
    }

    pub fn list_summaries(&self) -> Result<Vec<GameSaveSummary>, SaveStorageError> {
        let mut entries = self
            .read_manifest()?
            .entries
            .into_iter()
            .map(GameSaveSummary::from)
            .collect::<Vec<_>>();
        sort_save_summaries(&mut entries);
        Ok(entries)
    }

    pub fn create_new_save(
        &self,
        simulation: &Simulation,
    ) -> Result<GameSaveSummary, SaveStorageError> {
        let envelope = GameSaveEnvelope::capture_new(simulation)?;
        let summary = envelope.summary.clone();
        self.write_envelope(&envelope, false)?;
        Ok(summary)
    }

    pub fn overwrite_save(
        &self,
        save_id: &str,
        simulation: &Simulation,
    ) -> Result<GameSaveSummary, SaveStorageError> {
        let existing = self.find_summary(save_id)?;
        let envelope = GameSaveEnvelope::capture_overwrite(&existing, simulation)?;
        let summary = envelope.summary.clone();
        self.write_envelope(&envelope, true)?;
        Ok(summary)
    }

    pub fn load_save(&self, save_id: &str) -> Result<GameSaveEnvelope, SaveStorageError> {
        let summary = self.find_summary(save_id)?;
        let payload = self.read_payload(save_id)?;
        Ok(GameSaveEnvelope { summary, payload })
    }

    fn find_summary(&self, save_id: &str) -> Result<GameSaveSummary, SaveStorageError> {
        self.list_summaries()?
            .into_iter()
            .find(|summary| summary.id == save_id)
            .ok_or_else(|| SaveStorageError::NotFound(format!("Save slot {save_id} not found")))
    }

    fn write_envelope(
        &self,
        envelope: &GameSaveEnvelope,
        overwrite_existing: bool,
    ) -> Result<(), SaveStorageError> {
        let mut manifest = self.read_manifest()?;
        let metadata = StoredSaveMetadata::from(&envelope.summary);
        if let Some(existing) = manifest
            .entries
            .iter_mut()
            .find(|entry| entry.id == envelope.summary.id)
        {
            if !overwrite_existing {
                return Err(SaveStorageError::Io(format!(
                    "Save slot {} already exists",
                    envelope.summary.id
                )));
            }
            *existing = metadata;
        } else {
            manifest.entries.push(metadata);
        }
        manifest
            .entries
            .sort_by(|left, right| right.saved_at_unix.cmp(&left.saved_at_unix));
        self.write_payload(&envelope.summary.id, &envelope.payload)?;
        self.write_manifest(&manifest)
    }

    fn read_manifest(&self) -> Result<StorageManifest, SaveStorageError> {
        self.backend.read_manifest()
    }

    fn write_manifest(&self, manifest: &StorageManifest) -> Result<(), SaveStorageError> {
        self.backend.write_manifest(manifest)
    }

    fn read_payload(&self, save_id: &str) -> Result<String, SaveStorageError> {
        self.backend.read_payload(save_id)
    }

    fn write_payload(&self, save_id: &str, payload: &str) -> Result<(), SaveStorageError> {
        self.backend.write_payload(save_id, payload)
    }
}

impl Default for SaveStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl SaveStorageBackend {
    fn new() -> Self {
        #[cfg(target_arch = "wasm32")]
        {
            Self::Web
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            Self::Desktop(DesktopSaveStorage::default())
        }
    }

    #[cfg(test)]
    fn for_test_desktop_dir(save_dir: PathBuf) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        {
            Self::Desktop(DesktopSaveStorage {
                override_dir: Some(save_dir),
            })
        }

        #[cfg(target_arch = "wasm32")]
        {
            let _ = save_dir;
            Self::Web
        }
    }

    fn read_manifest(&self) -> Result<StorageManifest, SaveStorageError> {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Self::Desktop(storage) => storage.read_manifest(),
            #[cfg(target_arch = "wasm32")]
            Self::Web => read_web_manifest(),
        }
    }

    fn write_manifest(&self, manifest: &StorageManifest) -> Result<(), SaveStorageError> {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Self::Desktop(storage) => storage.write_manifest(manifest),
            #[cfg(target_arch = "wasm32")]
            Self::Web => write_web_manifest(manifest),
        }
    }

    fn read_payload(&self, save_id: &str) -> Result<String, SaveStorageError> {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Self::Desktop(storage) => storage.read_payload(save_id),
            #[cfg(target_arch = "wasm32")]
            Self::Web => read_web_payload(save_id),
        }
    }

    fn write_payload(&self, save_id: &str, payload: &str) -> Result<(), SaveStorageError> {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Self::Desktop(storage) => storage.write_payload(save_id, payload),
            #[cfg(target_arch = "wasm32")]
            Self::Web => write_web_payload(save_id, payload),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl DesktopSaveStorage {
    fn read_manifest(&self) -> Result<StorageManifest, SaveStorageError> {
        let path = self.save_dir()?.join(DESKTOP_MANIFEST_FILE);
        if !path.exists() {
            return Ok(StorageManifest::default());
        }
        let payload = fs::read_to_string(&path).map_err(|error| {
            SaveStorageError::Io(format!(
                "Failed to read save manifest {}: {error}",
                path.display()
            ))
        })?;
        serde_json::from_str(&payload).map_err(|error| {
            SaveStorageError::Parse(format!(
                "Failed to parse save manifest {}: {error}",
                path.display()
            ))
        })
    }

    fn write_manifest(&self, manifest: &StorageManifest) -> Result<(), SaveStorageError> {
        let save_dir = self.save_dir()?;
        fs::create_dir_all(&save_dir).map_err(|error| {
            SaveStorageError::Io(format!(
                "Failed to create save directory {}: {error}",
                save_dir.display()
            ))
        })?;
        let manifest_path = save_dir.join(DESKTOP_MANIFEST_FILE);
        let payload = serde_json::to_string_pretty(manifest).map_err(|error| {
            SaveStorageError::Parse(format!("Failed to serialize save manifest: {error}"))
        })?;
        fs::write(&manifest_path, format!("{payload}\n")).map_err(|error| {
            SaveStorageError::Io(format!(
                "Failed to write save manifest {}: {error}",
                manifest_path.display()
            ))
        })
    }

    fn read_payload(&self, save_id: &str) -> Result<String, SaveStorageError> {
        let path = self.payload_path(save_id)?;
        fs::read_to_string(&path).map_err(|error| {
            SaveStorageError::Io(format!(
                "Failed to read save payload {}: {error}",
                path.display()
            ))
        })
    }

    fn write_payload(&self, save_id: &str, payload: &str) -> Result<(), SaveStorageError> {
        let save_dir = self.save_dir()?;
        fs::create_dir_all(&save_dir).map_err(|error| {
            SaveStorageError::Io(format!(
                "Failed to create save directory {}: {error}",
                save_dir.display()
            ))
        })?;
        let payload_path = self.payload_path(save_id)?;
        fs::write(&payload_path, payload).map_err(|error| {
            SaveStorageError::Io(format!(
                "Failed to write save payload {}: {error}",
                payload_path.display()
            ))
        })
    }

    fn save_dir(&self) -> Result<PathBuf, SaveStorageError> {
        if let Some(path) = &self.override_dir {
            return Ok(path.clone());
        }

        if let Ok(executable_path) = std::env::current_exe() {
            if let Some(parent) = executable_path.parent() {
                return Ok(parent.join("saves"));
            }
        }

        std::env::current_dir()
            .map(|path| path.join("saves"))
            .map_err(|error| {
                SaveStorageError::Io(format!("Failed to resolve current directory: {error}"))
            })
    }

    fn payload_path(&self, save_id: &str) -> Result<PathBuf, SaveStorageError> {
        Ok(self.save_dir()?.join(format!("{save_id}.json")))
    }
}

#[cfg(target_arch = "wasm32")]
fn read_web_manifest() -> Result<StorageManifest, SaveStorageError> {
    let Some(storage) = web_storage()? else {
        return Ok(StorageManifest::default());
    };
    let payload = storage
        .get_item(storage_manifest_key())
        .map_err(js_error)?
        .unwrap_or_default();
    if payload.is_empty() {
        return Ok(StorageManifest::default());
    }
    serde_json::from_str(&payload).map_err(|error| {
        SaveStorageError::Parse(format!("Failed to parse web save manifest: {error}"))
    })
}

#[cfg(target_arch = "wasm32")]
fn write_web_manifest(manifest: &StorageManifest) -> Result<(), SaveStorageError> {
    let Some(storage) = web_storage()? else {
        return Err(SaveStorageError::Unavailable(
            "Browser localStorage is unavailable".to_string(),
        ));
    };
    let payload = serde_json::to_string(manifest).map_err(|error| {
        SaveStorageError::Parse(format!("Failed to serialize web save manifest: {error}"))
    })?;
    storage
        .set_item(storage_manifest_key(), &payload)
        .map_err(js_error)
}

#[cfg(target_arch = "wasm32")]
fn read_web_payload(save_id: &str) -> Result<String, SaveStorageError> {
    let Some(storage) = web_storage()? else {
        return Err(SaveStorageError::Unavailable(
            "Browser localStorage is unavailable".to_string(),
        ));
    };
    storage
        .get_item(&storage_payload_key(save_id))
        .map_err(js_error)?
        .ok_or_else(|| SaveStorageError::NotFound(format!("Save slot {save_id} not found")))
}

#[cfg(target_arch = "wasm32")]
fn write_web_payload(save_id: &str, payload: &str) -> Result<(), SaveStorageError> {
    let Some(storage) = web_storage()? else {
        return Err(SaveStorageError::Unavailable(
            "Browser localStorage is unavailable".to_string(),
        ));
    };
    storage
        .set_item(&storage_payload_key(save_id), payload)
        .map_err(js_error)
}

#[cfg(target_arch = "wasm32")]
fn web_storage() -> Result<Option<web_sys::Storage>, SaveStorageError> {
    let window = web_sys::window().ok_or_else(|| {
        SaveStorageError::Unavailable("Browser window is unavailable".to_string())
    })?;
    window.local_storage().map_err(js_error)
}

#[cfg(target_arch = "wasm32")]
fn js_error(error: web_sys::wasm_bindgen::JsValue) -> SaveStorageError {
    SaveStorageError::Unavailable(format!("Web storage error: {error:?}"))
}

impl From<StoredSaveMetadata> for GameSaveSummary {
    fn from(value: StoredSaveMetadata) -> Self {
        Self {
            id: value.id,
            display_name: value.display_name,
            saved_at_unix: value.saved_at_unix,
            world_time_label: value.world_time_label,
            capital: value.capital,
            debt: value.debt,
            reputation: value.reputation,
        }
    }
}

impl From<&GameSaveSummary> for StoredSaveMetadata {
    fn from(value: &GameSaveSummary) -> Self {
        Self {
            id: value.id.clone(),
            display_name: value.display_name.clone(),
            saved_at_unix: value.saved_at_unix,
            world_time_label: value.world_time_label.clone(),
            capital: value.capital,
            debt: value.debt,
            reputation: value.reputation,
        }
    }
}

pub fn storage_manifest_key() -> &'static str {
    STORAGE_MANIFEST_KEY
}

pub fn storage_payload_key(save_id: &str) -> String {
    format!("{STORAGE_PAYLOAD_PREFIX}{save_id}")
}

pub fn auto_save_name_from_timestamp(saved_at_unix: u64) -> String {
    format!("Save {}", format_utc_timestamp(saved_at_unix))
}

pub fn format_save_timestamp(saved_at_unix: u64) -> String {
    format_utc_timestamp(saved_at_unix)
}

pub fn sort_save_summaries(entries: &mut [GameSaveSummary]) {
    entries.sort_by(|left, right| {
        right
            .saved_at_unix
            .cmp(&left.saved_at_unix)
            .then_with(|| left.display_name.cmp(&right.display_name))
            .then_with(|| left.id.cmp(&right.id))
    });
}

pub fn toggle_save_menu(menu: &mut SaveMenuState, clock: &mut SimClock) {
    if menu.open {
        let paused_before_open = menu.paused_before_open.unwrap_or(false);
        menu.open = false;
        menu.pending_action = None;
        menu.last_error = None;
        menu.paused_before_open = None;
        clock.paused = paused_before_open;
        return;
    }

    menu.open = true;
    menu.pending_action = None;
    menu.last_error = None;
    menu.paused_before_open = Some(clock.paused);
    clock.paused = true;
}

pub fn refresh_save_entries(menu: &mut SaveMenuState, storage: &SaveStorage) {
    match storage.list_summaries() {
        Ok(entries) => {
            menu.entries = entries;
            if menu.selected_entry_id.as_ref().is_some_and(|selected_id| {
                menu.entries.iter().any(|entry| &entry.id == selected_id)
            }) {
                menu.last_error = None;
                return;
            }
            menu.selected_entry_id = menu.entries.first().map(|entry| entry.id.clone());
            menu.last_error = None;
        }
        Err(error) => {
            menu.entries.clear();
            menu.selected_entry_id = None;
            menu.last_error = Some(error.to_string());
        }
    }
}

pub fn toggle_save_menu_with_storage(
    menu: &mut SaveMenuState,
    clock: &mut SimClock,
    storage: &SaveStorage,
) {
    let opening = !menu.open;
    toggle_save_menu(menu, clock);
    if opening && menu.open {
        refresh_save_entries(menu, storage);
    }
}

pub fn save_menu_hotkey_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut menu: ResMut<SaveMenuState>,
    mut clock: ResMut<SimClock>,
    storage: Res<SaveStorage>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        toggle_save_menu_with_storage(&mut menu, &mut clock, &storage);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn apply_loaded_simulation(
    loaded: Simulation,
    loaded_name: &str,
    sim_resource: &mut SimResource,
    clock: &mut SimClock,
    camera: &mut CameraUiState,
    selected_system: &mut SelectedSystem,
    selected_station: &mut SelectedStation,
    selected_ship: &mut SelectedShip,
    panels: &mut UiPanelState,
    missions_panel: &mut MissionsPanelState,
    tracked_ship: &mut TrackedShip,
    ship_ui: &mut ShipUiState,
    station_ui: &mut StationUiState,
    markets_ui: &mut MarketsUiState,
    finance_ui: &mut FinanceUiState,
    kpi: &mut UiKpiTracker,
    messages: &mut HudMessages,
    menu: &mut SaveMenuState,
) {
    *sim_resource = SimResource::new(loaded);
    *clock = SimClock::default();
    clock.paused = true;
    *camera = CameraUiState::default();
    *selected_system = SelectedSystem::default();
    *selected_station = SelectedStation::default();
    *selected_ship = SelectedShip::default();
    *panels = UiPanelState::default();
    *missions_panel = MissionsPanelState::default();
    *tracked_ship = TrackedShip::default();
    *ship_ui = ShipUiState::default();
    *station_ui = StationUiState::default();
    *markets_ui = MarketsUiState::default();
    *finance_ui = FinanceUiState::default();
    *kpi = UiKpiTracker::default();
    *messages = HudMessages::default();
    messages.push(format!("Loaded save {loaded_name}"));
    *menu = SaveMenuState::default();
}

fn current_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn current_unix_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}

fn format_world_time_label(tick: u64, time: gatebound_sim::TimeSettingsView) -> String {
    let day_ticks = u64::from(time.day_ticks.max(1));
    let days_per_month = u64::from(time.days_per_month.max(1));
    let months_per_year = u64::from(time.months_per_year.max(1));
    let days_per_year = days_per_month.saturating_mul(months_per_year).max(1);

    let total_days = tick / day_ticks;
    let ticks_into_day = tick % day_ticks;
    let minutes_into_day = ticks_into_day.saturating_mul(24 * 60) / day_ticks;
    let hours = minutes_into_day / 60;
    let minutes = minutes_into_day % 60;
    let year = u64::from(time.start_year) + total_days / days_per_year;
    let month = (total_days / days_per_month) % months_per_year + 1;
    let day = total_days % days_per_month + 1;

    format!("{year:04}-{month:02}-{day:02} {hours:02}:{minutes:02}")
}

fn format_utc_timestamp(unix_seconds: u64) -> String {
    let seconds_per_day = 86_400_u64;
    let days = unix_seconds / seconds_per_day;
    let seconds_of_day = unix_seconds % seconds_per_day;
    let hours = seconds_of_day / 3_600;
    let minutes = (seconds_of_day % 3_600) / 60;
    let seconds = seconds_of_day % 60;
    let (year, month, day) = civil_from_days(days as i64);
    format!("{year:04}-{month:02}-{day:02} {hours:02}:{minutes:02}:{seconds:02} UTC")
}

fn civil_from_days(days_since_unix_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_unix_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if month <= 2 { 1 } else { 0 };
    (year as i32, month as u32, day as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_summary(id: &str, saved_at_unix: u64) -> GameSaveSummary {
        GameSaveSummary {
            id: id.to_string(),
            display_name: format!("Save {id}"),
            saved_at_unix,
            world_time_label: "3500-01-01 00:00".to_string(),
            capital: saved_at_unix as f64,
            debt: 0.0,
            reputation: 1.0,
        }
    }

    #[test]
    fn refresh_save_entries_keeps_selection_when_present() {
        let save_dir =
            std::env::temp_dir().join(format!("gatebound_save_refresh_{}", current_unix_nanos()));
        let storage = SaveStorage::for_test_desktop_dir(save_dir);
        let sim = Simulation::new(RuntimeConfig::default(), 13);
        let first = storage.create_new_save(&sim).expect("create should pass");
        let second = storage.create_new_save(&sim).expect("create should pass");

        let mut menu = SaveMenuState {
            open: true,
            selected_entry_id: Some(first.id.clone()),
            entries: vec![test_summary("stale", 1)],
            pending_action: Some(PendingSaveAction::Load(first.id.clone())),
            last_error: Some("stale".to_string()),
            paused_before_open: Some(false),
        };

        refresh_save_entries(&mut menu, &storage);

        assert_eq!(menu.entries.len(), 2);
        assert_eq!(menu.selected_entry_id, Some(first.id));
        assert!(menu.entries.iter().any(|entry| entry.id == second.id));
        assert!(menu.last_error.is_none());
    }
}
