//! CLI 进度条实现
//!
//! 这个模块提供基于 indicatif 的进度条实现，用于在 CLI 环境中显示进度

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use sftool_lib::progress::{ProgressCallback, ProgressId, ProgressInfo, ProgressType};
use std::collections::HashMap;
use std::io::{self, IsTerminal, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;

enum PercentProgressState {
    Spinner,
    Bar {
        total: u64,
        current: u64,
        last_percent: u64,
    },
}

/// 基于标准输出的百分比进度回调实现
pub struct PercentProgressCallback {
    progress_states: Arc<Mutex<HashMap<u64, PercentProgressState>>>,
    next_id: Arc<Mutex<u64>>,
}

impl PercentProgressCallback {
    /// 创建新的百分比进度回调
    pub fn new() -> Self {
        Self {
            progress_states: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(Mutex::new(1)),
        }
    }

    /// 获取下一个唯一的进度条 ID
    fn next_id(&self) -> u64 {
        let mut id = self.next_id.lock().unwrap();
        let current = *id;
        *id += 1;
        current
    }

    fn print_line(&self, line: &str) {
        let mut stdout = io::stdout();
        let _ = writeln!(stdout, "{}", line);
        let _ = stdout.flush();
    }

    fn calculate_percent(current: u64, total: u64) -> u64 {
        if total == 0 {
            return 100;
        }
        let percent = current.saturating_mul(100) / total;
        percent.min(100)
    }
}

impl ProgressCallback for PercentProgressCallback {
    fn start(&self, info: ProgressInfo) -> ProgressId {
        let id = self.next_id();
        let progress_id = ProgressId(id);

        let state = match info.progress_type {
            ProgressType::Spinner => {
                self.print_line("0%");
                PercentProgressState::Spinner
            }
            ProgressType::Bar { total } => {
                let current = info.current.unwrap_or(0);
                let percent = Self::calculate_percent(current, total);
                self.print_line(&format!("{}%", percent));
                PercentProgressState::Bar {
                    total,
                    current,
                    last_percent: percent,
                }
            }
        };

        self.progress_states.lock().unwrap().insert(id, state);
        progress_id
    }

    fn update_message(&self, _id: ProgressId, _message: String) {}

    fn increment(&self, id: ProgressId, delta: u64) {
        let mut percent_to_print = None;
        {
            let mut states = self.progress_states.lock().unwrap();
            if let Some(PercentProgressState::Bar {
                total,
                current,
                last_percent,
                ..
            }) = states.get_mut(&id.0)
            {
                *current = current.saturating_add(delta);
                let percent = Self::calculate_percent(*current, *total);
                if percent != *last_percent {
                    *last_percent = percent;
                    percent_to_print = Some(percent);
                }
            }
        }

        if let Some(percent) = percent_to_print {
            self.print_line(&format!("{}%", percent));
        }
    }

    fn finish(&self, id: ProgressId, final_message: String) {
        let aborted = final_message == "Aborted";
        let percent_to_print = {
            let mut states = self.progress_states.lock().unwrap();
            match states.remove(&id.0) {
                Some(PercentProgressState::Spinner) => {
                    if aborted {
                        None
                    } else {
                        Some(100)
                    }
                }
                Some(PercentProgressState::Bar { last_percent, .. }) => {
                    if aborted || last_percent == 100 {
                        None
                    } else {
                        Some(100)
                    }
                }
                None => None,
            }
        };

        if let Some(percent) = percent_to_print {
            self.print_line(&format!("{}%", percent));
        }
    }
}

/// 基于 indicatif 的进度回调实现
pub struct IndicatifProgressCallback {
    multi_progress: MultiProgress,
    progress_bars: Arc<Mutex<HashMap<u64, ProgressBar>>>,
    next_id: Arc<Mutex<u64>>,
}

impl IndicatifProgressCallback {
    /// 创建新的 indicatif 进度回调
    pub fn new() -> Self {
        Self {
            multi_progress: MultiProgress::new(),
            progress_bars: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(Mutex::new(1)),
        }
    }

    /// 获取下一个唯一的进度条 ID
    fn next_id(&self) -> u64 {
        let mut id = self.next_id.lock().unwrap();
        let current = *id;
        *id += 1;
        current
    }
}

impl Default for IndicatifProgressCallback {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgressCallback for IndicatifProgressCallback {
    fn start(&self, info: ProgressInfo) -> ProgressId {
        let id = self.next_id();
        let progress_id = ProgressId(id);

        let progress_bar = match info.progress_type {
            ProgressType::Spinner => {
                let spinner = self.multi_progress.add(ProgressBar::new_spinner());
                spinner.enable_steady_tick(Duration::from_millis(100));
                spinner.set_style(
                    ProgressStyle::with_template(&format!("[{}] {{spinner}} {{msg}}", info.prefix))
                        .unwrap_or_else(|_| ProgressStyle::default_spinner()),
                );
                spinner.set_message(info.message);
                spinner
            }
            ProgressType::Bar { total } => {
                let bar = self.multi_progress.add(ProgressBar::new(total));
                bar.set_style(
                    ProgressStyle::with_template(&format!(
                        "[{}] {{msg}} {{wide_bar}} {{bytes_per_sec}} {{percent_precise}}%",
                        info.prefix
                    ))
                    .unwrap_or_else(|_| ProgressStyle::default_bar())
                    .progress_chars("=>-"),
                );
                bar.set_message(info.message);
                if let Some(current) = info.current {
                    bar.set_position(current);
                }
                bar
            }
        };

        // 存储进度条引用
        self.progress_bars.lock().unwrap().insert(id, progress_bar);

        progress_id
    }

    fn update_message(&self, id: ProgressId, message: String) {
        if let Ok(bars) = self.progress_bars.lock()
            && let Some(bar) = bars.get(&id.0)
        {
            bar.set_message(message);
        }
    }

    fn increment(&self, id: ProgressId, delta: u64) {
        if let Ok(bars) = self.progress_bars.lock()
            && let Some(bar) = bars.get(&id.0)
        {
            bar.inc(delta);
        }
    }

    fn finish(&self, id: ProgressId, final_message: String) {
        if let Ok(mut bars) = self.progress_bars.lock()
            && let Some(bar) = bars.remove(&id.0)
        {
            bar.finish_with_message(final_message);
        }
    }
}

/// 创建 indicatif 进度回调的便利函数
pub fn create_indicatif_progress_callback() -> Arc<dyn ProgressCallback> {
    if io::stdout().is_terminal() {
        Arc::new(IndicatifProgressCallback::new())
    } else {
        Arc::new(PercentProgressCallback::new())
    }
}
