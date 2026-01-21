//! CLI 进度显示实现
//!
//! 这个模块负责将 lib 侧的结构化进度事件格式化并输出。

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use sftool_lib::progress::{
    EraseFlashStyle, EraseRegionStyle, ProgressContext, ProgressEvent, ProgressOperation,
    ProgressSink, ProgressStatus, ProgressType, StubStage,
};
use std::collections::HashMap;
use std::io::{self, IsTerminal, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn format_step(step: i32) -> String {
    format!("0x{:02X}", step)
}

fn size_to_u32(size: u64) -> u32 {
    size.min(u64::from(u32::MAX)) as u32
}

fn end_address(address: u32, size: u64) -> u32 {
    address.saturating_add(size_to_u32(size).saturating_sub(1))
}

/// CLI 侧格式化回调
pub trait ProgressFormatter: Send + Sync {
    fn start_message(&self, ctx: &ProgressContext) -> Option<String>;
    fn update_message(&self, ctx: &ProgressContext) -> Option<String>;
    fn finish_message(&self, ctx: &ProgressContext, status: &ProgressStatus) -> Option<String>;
}

/// 默认格式化器，复用原有文案
pub struct DefaultProgressFormatter;

impl DefaultProgressFormatter {
    fn format_operation_message(operation: &ProgressOperation) -> Option<String> {
        match operation {
            ProgressOperation::Connect => Some("Connecting to chip...".to_string()),
            ProgressOperation::DownloadStub { stage } => match stage {
                StubStage::Start => Some("Download stub...".to_string()),
                StubStage::SignatureKey => Some("Downloading signature key...".to_string()),
                StubStage::RamStub => Some("Downloading RAM stub...".to_string()),
            },
            ProgressOperation::EraseFlash { address, .. } => {
                Some(format!("Erasing entire flash at 0x{:08X}...", address))
            }
            ProgressOperation::EraseRegion {
                address,
                len,
                style,
            } => match style {
                EraseRegionStyle::LegacyFlashStartDecimalLength => {
                    Some(format!("Erasing entire flash at 0x{:08X}...", address))
                }
                EraseRegionStyle::HexLength | EraseRegionStyle::Range => Some(format!(
                    "Erasing region at 0x{:08X} (size: 0x{:08X})...",
                    address, len
                )),
            },
            ProgressOperation::EraseAllRegions => Some("Erasing all flash regions...".to_string()),
            ProgressOperation::Verify { .. } => Some("Verifying data...".to_string()),
            ProgressOperation::CheckRedownload { address, .. } => Some(format!(
                "Checking whether a re-download is necessary at address 0x{:08X}...",
                address
            )),
            ProgressOperation::WriteFlash { address, .. } => {
                Some(format!("Download at 0x{:08X}...", address))
            }
            ProgressOperation::ReadFlash { address, .. } => {
                Some(format!("Reading from 0x{:08X}...", address))
            }
        }
    }

    fn format_finish_message(
        operation: &ProgressOperation,
        status: &ProgressStatus,
    ) -> Option<String> {
        match operation {
            ProgressOperation::Connect => match status {
                ProgressStatus::Success => Some("Connected success!".to_string()),
                ProgressStatus::Retry => {
                    Some("Failed to connect to the chip, retrying...".to_string())
                }
                ProgressStatus::Failed(detail) => {
                    Some(format!("Failed to connect to the chip: {}", detail))
                }
                ProgressStatus::Aborted => Some("Aborted".to_string()),
                _ => None,
            },
            ProgressOperation::DownloadStub { .. } => match status {
                ProgressStatus::Success => Some("Download stub success!".to_string()),
                ProgressStatus::NotFound => {
                    Some("No stub file found for the given chip and memory type".to_string())
                }
                ProgressStatus::Failed(detail) => Some(format!("Download stub failed: {}", detail)),
                ProgressStatus::Aborted => Some("Aborted".to_string()),
                _ => None,
            },
            ProgressOperation::EraseFlash { address, style } => match status {
                ProgressStatus::Success => match style {
                    EraseFlashStyle::Complete => Some("Erase complete".to_string()),
                    EraseFlashStyle::Addressed => {
                        Some(format!("Erase flash successfully: 0x{:08X}", address))
                    }
                },
                ProgressStatus::Failed(detail) => Some(format!("Erase failed: {}", detail)),
                ProgressStatus::Aborted => Some("Aborted".to_string()),
                _ => None,
            },
            ProgressOperation::EraseRegion {
                address,
                len,
                style,
            } => match status {
                ProgressStatus::Success => match style {
                    EraseRegionStyle::LegacyFlashStartDecimalLength => Some(format!(
                        "Erase region successfully: 0x{:08X} (length: {} bytes)",
                        address, len
                    )),
                    EraseRegionStyle::HexLength => Some(format!(
                        "Erase region successfully: 0x{:08X} (length: 0x{:08X})",
                        address, len
                    )),
                    EraseRegionStyle::Range => Some(format!(
                        "Region erased successfully for 0x{:08X}..0x{:08X}",
                        address,
                        address.saturating_add(len.saturating_sub(1))
                    )),
                },
                ProgressStatus::Failed(detail) => Some(format!("Erase region failed: {}", detail)),
                ProgressStatus::Aborted => Some("Aborted".to_string()),
                _ => None,
            },
            ProgressOperation::EraseAllRegions => match status {
                ProgressStatus::Success => Some("All flash regions erased".to_string()),
                ProgressStatus::Failed(detail) => Some(format!("Erase failed: {}", detail)),
                ProgressStatus::Aborted => Some("Aborted".to_string()),
                _ => None,
            },
            ProgressOperation::Verify { .. } => match status {
                ProgressStatus::Success => Some("Verify success!".to_string()),
                ProgressStatus::Failed(detail) => Some(format!("Verify failed: {}", detail)),
                ProgressStatus::Aborted => Some("Aborted".to_string()),
                _ => None,
            },
            ProgressOperation::CheckRedownload { .. } => match status {
                ProgressStatus::Skipped => Some("No need to re-download, skip!".to_string()),
                ProgressStatus::Required => Some("Need to re-download".to_string()),
                ProgressStatus::Failed(detail) => {
                    Some(format!("Re-download check failed: {}", detail))
                }
                ProgressStatus::Aborted => Some("Aborted".to_string()),
                _ => None,
            },
            ProgressOperation::WriteFlash { address, size } => match status {
                ProgressStatus::Success => Some(format!(
                    "Downloaded successfully for 0x{:08X}..0x{:08X}",
                    address,
                    end_address(*address, *size)
                )),
                ProgressStatus::Failed(detail) => Some(format!("Download failed: {}", detail)),
                ProgressStatus::Aborted => Some("Aborted".to_string()),
                _ => None,
            },
            ProgressOperation::ReadFlash { .. } => match status {
                ProgressStatus::Success => Some("Read complete".to_string()),
                ProgressStatus::Failed(detail) => Some(format!("Read failed: {}", detail)),
                ProgressStatus::Aborted => Some("Aborted".to_string()),
                _ => None,
            },
        }
    }
}

impl ProgressFormatter for DefaultProgressFormatter {
    fn start_message(&self, ctx: &ProgressContext) -> Option<String> {
        Self::format_operation_message(&ctx.operation)
    }

    fn update_message(&self, ctx: &ProgressContext) -> Option<String> {
        Self::format_operation_message(&ctx.operation)
    }

    fn finish_message(&self, ctx: &ProgressContext, status: &ProgressStatus) -> Option<String> {
        Self::format_finish_message(&ctx.operation, status)
    }
}

enum PercentProgressState {
    Spinner {
        last_percent: u64,
    },
    Bar {
        total: u64,
        current: u64,
        last_percent: u64,
    },
}

/// 基于标准输出的百分比进度实现
pub struct PercentProgressSink {
    progress_states: Arc<Mutex<HashMap<u64, PercentProgressState>>>,
}

impl PercentProgressSink {
    pub fn new() -> Self {
        Self {
            progress_states: Arc::new(Mutex::new(HashMap::new())),
        }
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

impl ProgressSink for PercentProgressSink {
    fn on_event(&self, event: ProgressEvent) {
        match event {
            ProgressEvent::Start { id, ctx } => {
                let state = match ctx.progress_type {
                    ProgressType::Spinner => {
                        let percent = 0;
                        self.print_line(&format!("{}%", percent));
                        PercentProgressState::Spinner {
                            last_percent: percent,
                        }
                    }
                    ProgressType::Bar { total } => {
                        let current = ctx.current.unwrap_or(0);
                        let percent = Self::calculate_percent(current, total);
                        self.print_line(&format!("{}%", percent));
                        PercentProgressState::Bar {
                            total,
                            current,
                            last_percent: percent,
                        }
                    }
                };
                self.progress_states.lock().unwrap().insert(id.0, state);
            }
            ProgressEvent::Update { .. } => {}
            ProgressEvent::Advance { id, delta } => {
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
            ProgressEvent::Finish { id, status } => {
                let aborted = matches!(status, ProgressStatus::Aborted);
                let percent_to_print = {
                    let mut states = self.progress_states.lock().unwrap();
                    match states.remove(&id.0) {
                        Some(PercentProgressState::Spinner { last_percent }) => {
                            if aborted || last_percent == 100 {
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
    }
}

/// 基于 indicatif 的进度实现
pub struct IndicatifProgressSink {
    multi_progress: MultiProgress,
    progress_bars: Arc<Mutex<HashMap<u64, ProgressBar>>>,
    contexts: Arc<Mutex<HashMap<u64, ProgressContext>>>,
    formatter: Arc<dyn ProgressFormatter>,
}

impl IndicatifProgressSink {
    pub fn new(formatter: Arc<dyn ProgressFormatter>) -> Self {
        Self {
            multi_progress: MultiProgress::new(),
            progress_bars: Arc::new(Mutex::new(HashMap::new())),
            contexts: Arc::new(Mutex::new(HashMap::new())),
            formatter,
        }
    }

    fn set_message(&self, id: u64, message: Option<String>) {
        if let Some(message) = message
            && let Ok(bars) = self.progress_bars.lock()
            && let Some(bar) = bars.get(&id)
        {
            bar.set_message(message);
        }
    }
}

impl ProgressSink for IndicatifProgressSink {
    fn on_event(&self, event: ProgressEvent) {
        match event {
            ProgressEvent::Start { id, ctx } => {
                let prefix = format_step(ctx.step);
                let progress_bar = match ctx.progress_type {
                    ProgressType::Spinner => {
                        let spinner = self.multi_progress.add(ProgressBar::new_spinner());
                        spinner.enable_steady_tick(Duration::from_millis(100));
                        spinner.set_style(
                            ProgressStyle::with_template(&format!(
                                "[{}] {{spinner}} {{msg}}",
                                prefix
                            ))
                            .unwrap_or_else(|_| ProgressStyle::default_spinner()),
                        );
                        spinner
                    }
                    ProgressType::Bar { total } => {
                        let bar = self.multi_progress.add(ProgressBar::new(total));
                        bar.set_style(
                            ProgressStyle::with_template(&format!(
                                "[{}] {{msg}} {{wide_bar}} {{bytes_per_sec}} {{percent_precise}}%",
                                prefix
                            ))
                            .unwrap_or_else(|_| ProgressStyle::default_bar())
                            .progress_chars("=>-"),
                        );
                        if let Some(current) = ctx.current {
                            bar.set_position(current);
                        }
                        bar
                    }
                };

                let start_message = self.formatter.start_message(&ctx);
                if let Some(message) = start_message {
                    progress_bar.set_message(message);
                }

                self.progress_bars
                    .lock()
                    .unwrap()
                    .insert(id.0, progress_bar);
                self.contexts.lock().unwrap().insert(id.0, ctx);
            }
            ProgressEvent::Update { id, ctx } => {
                self.contexts.lock().unwrap().insert(id.0, ctx.clone());
                let message = self.formatter.update_message(&ctx);
                self.set_message(id.0, message);
            }
            ProgressEvent::Advance { id, delta } => {
                if let Ok(bars) = self.progress_bars.lock()
                    && let Some(bar) = bars.get(&id.0)
                {
                    bar.inc(delta);
                }
            }
            ProgressEvent::Finish { id, status } => {
                let ctx = self.contexts.lock().unwrap().remove(&id.0);
                let message = ctx
                    .as_ref()
                    .and_then(|ctx| self.formatter.finish_message(ctx, &status));

                if let Ok(mut bars) = self.progress_bars.lock()
                    && let Some(bar) = bars.remove(&id.0)
                {
                    if let Some(message) = message {
                        bar.finish_with_message(message);
                    } else {
                        bar.finish_and_clear();
                    }
                }
            }
        }
    }
}

pub fn create_progress_sink() -> Arc<dyn ProgressSink> {
    create_progress_sink_with_formatter(Arc::new(DefaultProgressFormatter))
}

pub fn create_progress_sink_with_formatter(
    formatter: Arc<dyn ProgressFormatter>,
) -> Arc<dyn ProgressSink> {
    if io::stdout().is_terminal() {
        Arc::new(IndicatifProgressSink::new(formatter))
    } else {
        Arc::new(PercentProgressSink::new())
    }
}
