//! 进度事件系统
//!
//! 这个模块定义了进度的结构化上下文与事件接口，便于在 CLI/GUI
//! 等不同前端进行统一格式化与呈现。

use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// 进度条类型
#[derive(Debug, Clone)]
pub enum ProgressType {
    /// 旋转进度条，用于不确定时长的操作
    Spinner,
    /// 条形进度条，用于有明确进度的操作
    Bar { total: u64 },
}

/// Stub 下载阶段
#[derive(Debug, Clone)]
pub enum StubStage {
    Start,
    SignatureKey,
    RamStub,
}

/// 整体擦除样式
#[derive(Debug, Clone)]
pub enum EraseFlashStyle {
    Complete,
    Addressed,
}

/// 区域擦除样式
#[derive(Debug, Clone)]
pub enum EraseRegionStyle {
    LegacyFlashStartDecimalLength,
    HexLength,
    Range,
}

/// 进度操作类型
#[derive(Debug, Clone)]
pub enum ProgressOperation {
    Connect,
    DownloadStub {
        stage: StubStage,
    },
    EraseFlash {
        address: u32,
        style: EraseFlashStyle,
    },
    EraseRegion {
        address: u32,
        len: u32,
        style: EraseRegionStyle,
    },
    EraseAllRegions,
    Verify {
        address: u32,
        len: u32,
    },
    CheckRedownload {
        address: u32,
        size: u64,
    },
    WriteFlash {
        address: u32,
        size: u64,
    },
    ReadFlash {
        address: u32,
        size: u32,
    },
}

/// 进度上下文
#[derive(Debug, Clone)]
pub struct ProgressContext {
    /// 步骤号
    pub step: i32,
    /// 进度条类型
    pub progress_type: ProgressType,
    /// 操作语义
    pub operation: ProgressOperation,
    /// 当前进度（仅对 Bar 类型有效）
    pub current: Option<u64>,
}

/// 进度条 ID 类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProgressId(pub u64);

/// 进度完成状态
#[derive(Debug, Clone)]
pub enum ProgressStatus {
    Success,
    Retry,
    Skipped,
    Required,
    NotFound,
    Failed(String),
    Aborted,
}

/// 进度事件
#[derive(Debug, Clone)]
pub enum ProgressEvent {
    Start {
        id: ProgressId,
        ctx: ProgressContext,
    },
    Update {
        id: ProgressId,
        ctx: ProgressContext,
    },
    Advance {
        id: ProgressId,
        delta: u64,
    },
    Finish {
        id: ProgressId,
        status: ProgressStatus,
    },
}

/// 进度事件接收器
pub trait ProgressSink: Send + Sync {
    fn on_event(&self, event: ProgressEvent);
}

/// 进度事件接收器的包装器
pub type ProgressSinkArc = Arc<dyn ProgressSink>;

/// 默认的空进度接收器实现
#[derive(Debug, Default)]
pub struct NoOpProgressSink;

impl ProgressSink for NoOpProgressSink {
    fn on_event(&self, _event: ProgressEvent) {}
}

/// 创建默认的空进度接收器
pub fn no_op_progress_sink() -> ProgressSinkArc {
    Arc::new(NoOpProgressSink)
}

/// 进度助手结构体
pub struct ProgressHelper {
    sink: ProgressSinkArc,
    step_counter: Arc<AtomicI32>,
    id_counter: Arc<AtomicU64>,
}

impl ProgressHelper {
    /// 创建新的进度助手，从指定的初始步骤开始
    pub fn new(sink: ProgressSinkArc, initial_step: i32) -> Self {
        Self {
            sink,
            step_counter: Arc::new(AtomicI32::new(initial_step)),
            id_counter: Arc::new(AtomicU64::new(1)),
        }
    }

    /// 获取下一个步骤号并递增计数器
    fn next_step(&self) -> i32 {
        self.step_counter.fetch_add(1, Ordering::SeqCst)
    }

    /// 获取下一个进度条 ID
    fn next_id(&self) -> ProgressId {
        ProgressId(self.id_counter.fetch_add(1, Ordering::SeqCst))
    }

    /// 创建一个旋转进度条
    pub fn create_spinner(&self, operation: ProgressOperation) -> ProgressHandle {
        let step = self.next_step();
        let id = self.next_id();
        let ctx = ProgressContext {
            step,
            progress_type: ProgressType::Spinner,
            operation,
            current: None,
        };
        self.sink.on_event(ProgressEvent::Start {
            id,
            ctx: ctx.clone(),
        });
        ProgressHandle::new(Arc::clone(&self.sink), id, ctx)
    }

    /// 创建一个条形进度条
    pub fn create_bar(&self, total: u64, operation: ProgressOperation) -> ProgressHandle {
        let step = self.next_step();
        let id = self.next_id();
        let ctx = ProgressContext {
            step,
            progress_type: ProgressType::Bar { total },
            operation,
            current: Some(0),
        };
        self.sink.on_event(ProgressEvent::Start {
            id,
            ctx: ctx.clone(),
        });
        ProgressHandle::new(Arc::clone(&self.sink), id, ctx)
    }

    /// 获取当前步骤号（不递增）
    pub fn current_step(&self) -> i32 {
        self.step_counter.load(Ordering::SeqCst)
    }

    /// 同步步骤计数器到外部计数器
    pub fn sync_step_to_external(&self, external_step: &mut i32) {
        *external_step = self.current_step();
    }
}

/// 进度条处理器
pub struct ProgressHandle {
    sink: ProgressSinkArc,
    id: ProgressId,
    context: Mutex<ProgressContext>,
    finished: bool,
}

impl ProgressHandle {
    fn new(sink: ProgressSinkArc, id: ProgressId, context: ProgressContext) -> Self {
        Self {
            sink,
            id,
            context: Mutex::new(context),
            finished: false,
        }
    }

    /// 更新操作语义
    pub fn set_operation(&self, operation: ProgressOperation) {
        let mut ctx = self.context.lock().unwrap();
        ctx.operation = operation;
        self.sink.on_event(ProgressEvent::Update {
            id: self.id,
            ctx: ctx.clone(),
        });
    }

    /// 增加进度
    pub fn inc(&self, delta: u64) {
        self.sink
            .on_event(ProgressEvent::Advance { id: self.id, delta });
    }

    /// 完成进度条
    pub fn finish(mut self, status: ProgressStatus) {
        self.finished = true;
        self.sink.on_event(ProgressEvent::Finish {
            id: self.id,
            status,
        });
    }
}

impl Drop for ProgressHandle {
    fn drop(&mut self) {
        if !self.finished {
            self.sink.on_event(ProgressEvent::Finish {
                id: self.id,
                status: ProgressStatus::Aborted,
            });
        }
    }
}
