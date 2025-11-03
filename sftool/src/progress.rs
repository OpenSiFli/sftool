//! CLI 进度条实现
//!
//! 这个模块提供基于 indicatif 的进度条实现，用于在 CLI 环境中显示进度

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use sftool_lib::progress::{ProgressCallback, ProgressId, ProgressInfo, ProgressType};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

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
    Arc::new(IndicatifProgressCallback::new())
}
