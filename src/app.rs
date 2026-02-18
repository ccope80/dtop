use crate::alerts::{self, Alert};
use crate::collectors::{diskstats, filesystem, lsblk, lvm, mdraid, nfs, process_io, smart as smart_collector, smart_cache, zfs};
use crate::util::{alert_log, webhook};
use crate::config::Config;
use crate::input::{handle_key, Action};
use crate::models::device::BlockDevice;
use crate::models::filesystem::Filesystem;
use crate::models::process::{ProcessIORates, ProcessSort, RawProcessIO};
use crate::models::smart::SmartData;
use crate::models::volume::{LvmState, RaidArray, ZfsPool};
use crate::ui::theme::{Theme, ThemeVariant};
use crate::ui::{dashboard, filesystem_view, help, nfs_view, process_view, volume_view};
use crate::util::ring_buffer::RingBuffer;
use anyhow::Result;
use crossterm::event::{self, Event, MouseButton, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::widgets::{ListState, TableState};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::mpsc;
use std::time::{Duration, Instant};

// ── View / Panel enums ────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum ActiveView {
    Dashboard,
    ProcessIO,
    FilesystemOverview,
    VolumeManager,
    NfsView,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ActivePanel {
    Devices,
    Throughput,
    Filesystem,
    SmartTemp,
    Alerts,
    Detail,
}

// ── Tick intervals ────────────────────────────────────────────────────

const FAST_TICK:    Duration = Duration::from_millis(2000);
const SLOW_TICK:    Duration = Duration::from_millis(30_000);
const SMART_TICK:   Duration = Duration::from_secs(300);
const POLL_TIMEOUT: Duration = Duration::from_millis(150);

// ── Background SMART result ───────────────────────────────────────────

struct SmartResult {
    device_name: String,
    data:        Option<SmartData>,
}

// ── App ───────────────────────────────────────────────────────────────

pub struct App {
    // Config
    pub config: Config,

    // Theme
    pub theme:         Theme,
    pub theme_variant: ThemeVariant,

    // View routing
    pub active_view:  ActiveView,
    pub active_panel: ActivePanel,

    // Layout preset (0=Full, 1=IO-Focus, 2=Storage)
    pub layout_preset: usize,

    // Help overlay
    pub show_help: bool,

    // Dashboard state
    pub device_list_state: ListState,
    pub device_list_area:  Option<Rect>,
    pub detail_open:       bool,
    pub detail_scroll:     usize,
    pub detail_history_window: usize,   // 0=60s, 1=5m, 2=1h
    pub fs_scroll:         usize,

    // F3 filesystem overview state
    pub fs_table_state: TableState,

    // F2 process I/O state
    pub process_table_state: TableState,
    pub process_sort:        ProcessSort,

    // F4 volume manager state
    pub volume_scroll: usize,

    // Core data
    pub devices:     Vec<BlockDevice>,
    pub filesystems: Vec<Filesystem>,
    pub alerts:      Vec<Alert>,

    // Alert history ring — (timestamp_str, Alert)
    pub alert_history: VecDeque<(String, Alert)>,

    // Process I/O data
    pub process_io:         Vec<ProcessIORates>,
    pub proc_read_history:  RingBuffer,
    pub proc_write_history: RingBuffer,

    // Volume manager data
    pub raid_arrays: Vec<RaidArray>,
    pub lvm_state:   Option<LvmState>,
    pub zfs_pools:   Vec<ZfsPool>,

    // NFS mount data (F5)
    pub nfs_mounts: Vec<nfs::NfsMountStats>,

    // Internal: previous diskstats for delta
    prev_diskstats:  HashMap<String, diskstats::RawDiskstat>,
    prev_process_io: HashMap<u32, RawProcessIO>,
    uid_cache:       HashMap<u32, String>,

    last_fast_tick:  Instant,
    last_slow_tick:  Instant,
    last_smart_tick: Instant,

    // Background SMART polling
    smart_tx:      mpsc::Sender<SmartResult>,
    smart_rx:      mpsc::Receiver<SmartResult>,
    smart_pending: HashSet<String>,

    pub should_quit: bool,
}

impl App {
    pub fn new(initial_theme: ThemeVariant) -> Result<Self> {
        let (smart_tx, smart_rx) = mpsc::channel();
        let config = Config::load();

        let mut app = Self {
            config,
            theme:         Theme::for_variant(initial_theme),
            theme_variant: initial_theme,
            active_view:   ActiveView::Dashboard,
            active_panel:  ActivePanel::Devices,
            layout_preset: 0,
            show_help:     false,
            device_list_state:     ListState::default(),
            device_list_area:      None,
            detail_open:           false,
            detail_scroll:         0,
            detail_history_window: 0,
            fs_scroll:             0,
            fs_table_state:        TableState::default(),
            process_table_state:   TableState::default(),
            process_sort:          ProcessSort::WritePerSec,
            volume_scroll:         0,
            devices:       Vec::new(),
            filesystems:   Vec::new(),
            alerts:        Vec::new(),
            alert_history: VecDeque::new(),
            process_io:    Vec::new(),
            proc_read_history:  RingBuffer::new(300),
            proc_write_history: RingBuffer::new(300),
            raid_arrays:   Vec::new(),
            lvm_state:     None,
            zfs_pools:     Vec::new(),
            nfs_mounts:    Vec::new(),
            prev_diskstats:  HashMap::new(),
            prev_process_io: HashMap::new(),
            uid_cache:       HashMap::new(),
            last_fast_tick:  Instant::now() - FAST_TICK,
            last_slow_tick:  Instant::now() - SLOW_TICK,
            last_smart_tick: Instant::now() - SMART_TICK,
            smart_tx,
            smart_rx,
            smart_pending: HashSet::new(),
            should_quit:   false,
        };

        app.collect_slow()?;
        app.collect_fast()?;

        // Seed SMART data from disk cache so health status is shown immediately
        let cache = smart_cache::load();
        for dev in &mut app.devices {
            if let Some(cached) = cache.get(&dev.name) {
                dev.smart = Some(cached.clone());
                if let Some(t) = cached.temperature {
                    dev.temp_history.push(t as u64);
                }
            }
        }

        app.schedule_all_smart();

        if !app.devices.is_empty() {
            app.device_list_state.select(Some(0));
        }
        Ok(app)
    }

    // ── Main event loop ───────────────────────────────────────────────

    pub fn run<B: ratatui::backend::Backend>(
        &mut self,
        terminal: &mut ratatui::Terminal<B>,
    ) -> Result<()> {
        loop {
            self.consume_smart_results();

            let show_help  = self.show_help;
            let theme_snap = self.theme.clone();

            terminal.draw(|f| {
                match self.active_view {
                    ActiveView::Dashboard          => dashboard::render(f, self),
                    ActiveView::ProcessIO          => process_view::render(f, self),
                    ActiveView::FilesystemOverview => filesystem_view::render(f, self),
                    ActiveView::VolumeManager      => volume_view::render(f, self),
                    ActiveView::NfsView            => nfs_view::render(f, self),
                }
                if show_help {
                    help::render(f, &theme_snap);
                }
            })?;

            if event::poll(POLL_TIMEOUT)? {
                match event::read()? {
                    Event::Key(key) => {
                        let action = handle_key(key);
                        self.handle_action(action);
                    }
                    Event::Mouse(me) => match me.kind {
                        MouseEventKind::ScrollDown => self.handle_action(Action::ScrollDown),
                        MouseEventKind::ScrollUp   => self.handle_action(Action::ScrollUp),
                        MouseEventKind::Down(MouseButton::Left) => {
                            self.handle_mouse_click(me.column, me.row);
                        }
                        _ => {}
                    },
                    Event::Resize(_, _) => {}
                    _ => {}
                }
            }

            if self.should_quit { break; }

            if self.last_fast_tick.elapsed() >= FAST_TICK {
                let prev_alerts = self.alerts.clone();
                self.collect_fast()?;
                self.last_fast_tick = Instant::now();
                let new_alerts = alerts::evaluate(
                    &self.devices, &self.filesystems,
                    &self.config.alerts.thresholds,
                );
                self.update_alert_history(&prev_alerts, &new_alerts);
                self.alerts = new_alerts;
            }

            if self.last_slow_tick.elapsed() >= SLOW_TICK {
                self.collect_slow()?;
                self.last_slow_tick = Instant::now();
            }

            if self.last_smart_tick.elapsed() >= SMART_TICK {
                self.schedule_all_smart();
                self.last_smart_tick = Instant::now();
            }
        }
        Ok(())
    }

    // ── Alert history ──────────────────────────────────────────────────

    fn update_alert_history(&mut self, prev: &[Alert], new: &[Alert]) {
        let now = chrono::Local::now().format("%H:%M:%S").to_string();
        let mut fresh: Vec<Alert> = Vec::new();

        for alert in new {
            let key = format!("{}{}{}", alert.severity.label(), alert.prefix(), alert.message);
            let was_present = prev.iter().any(|a| {
                format!("{}{}{}", a.severity.label(), a.prefix(), a.message) == key
            });
            if !was_present {
                if self.alert_history.len() >= 50 {
                    self.alert_history.pop_back();
                }
                self.alert_history.push_front((now.clone(), alert.clone()));
                fresh.push(alert.clone());
            }
        }

        if !fresh.is_empty() {
            alert_log::append(&fresh);
            if !self.config.notifications.webhook_url.is_empty() {
                webhook::notify(
                    &fresh,
                    &self.config.notifications.webhook_url.clone(),
                    self.config.notifications.notify_warning,
                );
            }
        }
    }

    // ── Input dispatch ────────────────────────────────────────────────

    fn handle_action(&mut self, action: Action) {
        if self.show_help {
            match action {
                Action::Quit     => self.should_quit = true,
                Action::ShowHelp | Action::Back => { self.show_help = false; }
                _ => {}
            }
            return;
        }

        match action {
            Action::Quit => self.should_quit = true,

            Action::ShowHelp => { self.show_help = true; }

            Action::CycleTheme => {
                self.theme_variant = self.theme_variant.next();
                self.theme = Theme::for_variant(self.theme_variant);
            }

            Action::CyclePreset => {
                if self.active_view == ActiveView::Dashboard && !self.detail_open {
                    self.layout_preset = (self.layout_preset + 1) % 3;
                }
            }

            Action::CycleWindow => {
                if self.detail_open {
                    self.detail_history_window = (self.detail_history_window + 1) % 3;
                }
            }

            Action::ViewProcessIO => {
                self.active_view = if self.active_view == ActiveView::ProcessIO {
                    ActiveView::Dashboard
                } else {
                    ActiveView::ProcessIO
                };
            }
            Action::ViewFilesystem => {
                self.active_view = if self.active_view == ActiveView::FilesystemOverview {
                    ActiveView::Dashboard
                } else {
                    ActiveView::FilesystemOverview
                };
            }
            Action::ViewVolume => {
                self.active_view = if self.active_view == ActiveView::VolumeManager {
                    ActiveView::Dashboard
                } else {
                    ActiveView::VolumeManager
                };
            }
            Action::ViewNfs => {
                self.active_view = if self.active_view == ActiveView::NfsView {
                    ActiveView::Dashboard
                } else {
                    ActiveView::NfsView
                };
            }

            Action::FocusNext => {
                if self.active_view == ActiveView::Dashboard { self.cycle_focus(1); }
            }
            Action::FocusPrev => {
                if self.active_view == ActiveView::Dashboard { self.cycle_focus(-1); }
            }

            Action::SelectUp   => self.select_delta(-1),
            Action::SelectDown => self.select_delta(1),

            Action::Confirm => {
                if self.active_view == ActiveView::Dashboard
                    && self.active_panel == ActivePanel::Devices
                    && !self.detail_open
                {
                    self.detail_open           = true;
                    self.detail_scroll         = 0;
                    self.detail_history_window = 0;
                    self.active_panel          = ActivePanel::Detail;
                    if let Some(idx) = self.device_list_state.selected() {
                        if let Some(dev) = self.devices.get(idx) {
                            self.schedule_smart(&dev.name.clone());
                        }
                    }
                }
            }

            Action::Back => {
                if self.active_view != ActiveView::Dashboard {
                    self.active_view = ActiveView::Dashboard;
                } else {
                    self.detail_open   = false;
                    self.detail_scroll = 0;
                    self.active_panel  = ActivePanel::Devices;
                }
            }

            Action::CycleSort => {
                if self.active_view == ActiveView::ProcessIO {
                    self.process_sort = self.process_sort.next();
                    self.sort_processes();
                } else if self.detail_open {
                    if let Some(idx) = self.device_list_state.selected() {
                        if let Some(dev) = self.devices.get(idx) {
                            self.schedule_smart(&dev.name.clone());
                        }
                    }
                }
            }

            Action::SmartRefresh => {}

            Action::ScrollUp => match self.active_view {
                ActiveView::Dashboard => match self.active_panel {
                    ActivePanel::Detail     => self.detail_scroll = self.detail_scroll.saturating_sub(1),
                    ActivePanel::Filesystem => self.fs_scroll = self.fs_scroll.saturating_sub(1),
                    _ => self.select_delta(-1),
                },
                ActiveView::ProcessIO => {
                    let cur = self.process_table_state.selected().unwrap_or(0);
                    if cur > 0 { self.process_table_state.select(Some(cur - 1)); }
                }
                ActiveView::FilesystemOverview => {
                    let cur = self.fs_table_state.selected().unwrap_or(0);
                    if cur > 0 { self.fs_table_state.select(Some(cur - 1)); }
                }
                ActiveView::VolumeManager => {
                    self.volume_scroll = self.volume_scroll.saturating_sub(1);
                }
                ActiveView::NfsView => {}
            },

            Action::ScrollDown => match self.active_view {
                ActiveView::Dashboard => match self.active_panel {
                    ActivePanel::Detail => { self.detail_scroll += 1; }
                    ActivePanel::Filesystem => {
                        let max = self.filesystems.len().saturating_sub(1);
                        if self.fs_scroll < max { self.fs_scroll += 1; }
                    }
                    _ => self.select_delta(1),
                },
                ActiveView::ProcessIO => {
                    let max = self.process_io.len().saturating_sub(1);
                    let cur = self.process_table_state.selected().unwrap_or(0);
                    if cur < max { self.process_table_state.select(Some(cur + 1)); }
                }
                ActiveView::FilesystemOverview => {
                    let max = self.filesystems.len().saturating_sub(1);
                    let cur = self.fs_table_state.selected().unwrap_or(0);
                    if cur < max { self.fs_table_state.select(Some(cur + 1)); }
                }
                ActiveView::VolumeManager => { self.volume_scroll += 1; }
                ActiveView::NfsView => {}
            },

            Action::None => {}
        }
    }

    // ── Mouse click handling ──────────────────────────────────────────

    fn handle_mouse_click(&mut self, _col: u16, row: u16) {
        if self.active_view != ActiveView::Dashboard { return; }
        if let Some(area) = self.device_list_area {
            let top = area.y + 1;
            let bot = area.y + area.height.saturating_sub(1);
            if row >= top && row < bot {
                let offset = self.device_list_state.offset();
                let idx = (row - top) as usize + offset;
                if idx < self.devices.len() {
                    self.device_list_state.select(Some(idx));
                    self.active_panel = ActivePanel::Devices;
                }
            }
        }
    }

    fn cycle_focus(&mut self, dir: i32) {
        if self.detail_open { return; }
        let panels = [
            ActivePanel::Devices,
            ActivePanel::Throughput,
            ActivePanel::Filesystem,
            ActivePanel::SmartTemp,
            ActivePanel::Alerts,
        ];
        let cur  = panels.iter().position(|p| p == &self.active_panel).unwrap_or(0);
        let next = ((cur as i32 + dir).rem_euclid(panels.len() as i32)) as usize;
        self.active_panel = panels[next].clone();
    }

    fn select_delta(&mut self, delta: i32) {
        if self.devices.is_empty() { return; }
        let cur  = self.device_list_state.selected().unwrap_or(0) as i32;
        let next = (cur + delta).clamp(0, self.devices.len() as i32 - 1) as usize;
        self.device_list_state.select(Some(next));
    }

    // ── Fast data collection (2 s) ────────────────────────────────────

    fn collect_fast(&mut self) -> Result<()> {
        let now_stats = diskstats::read_diskstats()?;
        let elapsed   = self.last_fast_tick.elapsed().as_secs_f64().max(0.001);

        for dev in &mut self.devices {
            if let (Some(prev), Some(curr)) = (
                self.prev_diskstats.get(&dev.name),
                now_stats.get(&dev.name),
            ) {
                let io = diskstats::compute_io(prev, curr, elapsed, curr.ios_in_progress);
                dev.read_bytes_per_sec   = io.read_bytes_per_sec;
                dev.write_bytes_per_sec  = io.write_bytes_per_sec;
                dev.read_iops            = io.read_iops;
                dev.write_iops           = io.write_iops;
                dev.io_util_pct          = io.io_util_pct;
                dev.avg_read_latency_ms  = io.avg_read_latency_ms;
                dev.avg_write_latency_ms = io.avg_write_latency_ms;
                dev.read_history .push((io.read_bytes_per_sec  / 1024.0) as u64);
                dev.write_history.push((io.write_bytes_per_sec / 1024.0) as u64);
                dev.util_history .push(io.io_util_pct as u64);
                // Latency stored as µs (×1000) for better sparkline resolution
                dev.read_lat_history .push((io.avg_read_latency_ms  * 1000.0) as u64);
                dev.write_lat_history.push((io.avg_write_latency_ms * 1000.0) as u64);
            } else if now_stats.contains_key(&dev.name) {
                dev.read_history .push(0);
                dev.write_history.push(0);
                dev.util_history .push(0);
                dev.read_lat_history .push(0);
                dev.write_lat_history.push(0);
            }
        }

        if let Ok(fs) = filesystem::read_filesystems() {
            self.filesystems = fs;
        }

        // Process I/O
        let curr_proc = process_io::read_all();
        let mut rates = process_io::compute_rates(
            &self.prev_process_io, &curr_proc, elapsed, &mut self.uid_cache,
        );
        self.sort_processes_vec(&mut rates);
        let total_r: f64 = rates.iter().map(|p| p.read_per_sec).sum();
        let total_w: f64 = rates.iter().map(|p| p.write_per_sec).sum();
        self.proc_read_history .push((total_r / 1024.0) as u64);
        self.proc_write_history.push((total_w / 1024.0) as u64);
        self.process_io      = rates;
        self.prev_process_io = curr_proc;

        // NFS mounts (cheap read of /proc/self/mountstats)
        self.nfs_mounts = nfs::read_nfs_mounts();

        self.prev_diskstats = now_stats;
        Ok(())
    }

    // ── Slow data collection (30 s) ───────────────────────────────────

    fn collect_slow(&mut self) -> Result<()> {
        let lsblk_devs = lsblk::run_lsblk().unwrap_or_default();
        let raw        = diskstats::read_diskstats().unwrap_or_default();
        let mut new_devices: Vec<BlockDevice> = Vec::new();

        for raw_name in raw.keys() {
            let existing_pos = self.devices.iter().position(|d| &d.name == raw_name);
            let mut dev = if let Some(pos) = existing_pos {
                self.devices.remove(pos)
            } else {
                BlockDevice::new(raw_name.clone())
            };

            if let Some(lb) = lsblk_devs.iter().find(|l| &l.name == raw_name) {
                dev.model          = lb.model.clone();
                dev.serial         = lb.serial.clone();
                dev.capacity_bytes = lb.size;
                dev.rotational     = lb.rotational;
                dev.transport      = lb.transport.clone();
                dev.partitions     = lb.partitions.clone();
            }
            dev.infer_type();
            new_devices.push(dev);
        }

        new_devices.sort_by(|a, b| {
            type_order(&a.dev_type).cmp(&type_order(&b.dev_type)).then(a.name.cmp(&b.name))
        });

        let selected_name = self.device_list_state.selected()
            .and_then(|i| self.devices.get(i))
            .map(|d| d.name.clone());

        self.devices = new_devices;

        if let Some(name) = selected_name {
            if let Some(pos) = self.devices.iter().position(|d| d.name == name) {
                self.device_list_state.select(Some(pos));
            }
        }
        if self.device_list_state.selected().is_none() && !self.devices.is_empty() {
            self.device_list_state.select(Some(0));
        }

        self.raid_arrays = mdraid::read_mdstat();
        self.lvm_state   = lvm::read_lvm();
        self.zfs_pools   = zfs::read_zpools();

        Ok(())
    }

    // ── SMART background polling ──────────────────────────────────────

    fn schedule_smart(&mut self, name: &str) {
        if self.smart_pending.contains(name) { return; }
        self.smart_pending.insert(name.to_string());
        let tx   = self.smart_tx.clone();
        let name = name.to_string();
        std::thread::spawn(move || {
            let data = smart_collector::poll_device(&name);
            let _ = tx.send(SmartResult { device_name: name, data });
        });
    }

    fn schedule_all_smart(&mut self) {
        let names: Vec<String> = self.devices.iter().map(|d| d.name.clone()).collect();
        for name in names { self.schedule_smart(&name); }
    }

    fn consume_smart_results(&mut self) {
        let mut cache_dirty = false;
        while let Ok(result) = self.smart_rx.try_recv() {
            self.smart_pending.remove(&result.device_name);
            if let Some(dev) = self.devices.iter_mut().find(|d| d.name == result.device_name) {
                dev.smart_prev      = dev.smart.clone();
                dev.smart           = result.data;
                dev.smart_polled_at = Some(Instant::now());
                // Push temperature into history
                if let Some(t) = dev.temperature() {
                    dev.temp_history.push(t as u64);
                }
                cache_dirty = true;
            }
        }
        // Persist SMART cache after any updates
        if cache_dirty {
            let cache: smart_cache::SmartCache = self.devices.iter()
                .filter_map(|d| d.smart.as_ref().map(|s| (d.name.clone(), s.clone())))
                .collect();
            smart_cache::save(&cache);
        }
    }

    // ── Process sort ──────────────────────────────────────────────────

    fn sort_processes(&mut self) {
        let sort = self.process_sort.clone();
        Self::sort_by(&mut self.process_io, &sort);
    }

    fn sort_processes_vec(&self, v: &mut Vec<ProcessIORates>) {
        Self::sort_by(v, &self.process_sort);
    }

    fn sort_by(v: &mut Vec<ProcessIORates>, sort: &ProcessSort) {
        match sort {
            ProcessSort::WritePerSec => v.sort_by(|a, b| b.write_per_sec.partial_cmp(&a.write_per_sec).unwrap()),
            ProcessSort::ReadPerSec  => v.sort_by(|a, b| b.read_per_sec .partial_cmp(&a.read_per_sec ).unwrap()),
            ProcessSort::Total       => v.sort_by(|a, b| b.total_per_sec().partial_cmp(&a.total_per_sec()).unwrap()),
            ProcessSort::Pid         => v.sort_by_key(|p| p.pid),
            ProcessSort::Name        => v.sort_by(|a, b| a.comm.cmp(&b.comm)),
        }
    }
}

fn type_order(t: &crate::models::device::DeviceType) -> u8 {
    use crate::models::device::DeviceType::*;
    match t { NVMe => 0, SSD => 1, HDD => 2, Virtual => 3, Unknown => 4 }
}
