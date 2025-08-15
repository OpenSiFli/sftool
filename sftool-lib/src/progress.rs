//! 进度条回调系统
//!
//! 这个模块定义了进度条的抽象接口，允许用户在不同环境（CLI、GUI等）中
//! 自定义进度条的显示方式。

use std::sync::Arc;

/// 进度条类型
#[derive(Debug, Clone)]
pub enum ProgressType {
    /// 旋转进度条，用于不确定时长的操作
    Spinner,
    /// 条形进度条，用于有明确进度的操作
    Bar { total: u64 },
}

/// 进度条状态
#[derive(Debug, Clone)]
pub struct ProgressInfo {
    /// 进度条类型
    pub progress_type: ProgressType,
    /// 步骤前缀（通常是十六进制步骤号）
    pub prefix: String,
    /// 当前消息
    pub message: String,
    /// 当前进度（仅对 Bar 类型有效）
    pub current: Option<u64>,
}

/// 进度回调 trait
///
/// 实现此 trait 以自定义进度条的显示方式
pub trait ProgressCallback: Send + Sync {
    /// 开始一个新的进度条
    ///
    /// # 参数
    /// - `info`: 进度条信息
    ///
    /// # 返回值
    /// 返回一个进度条 ID，用于后续的更新和完成操作
    fn start(&self, info: ProgressInfo) -> ProgressId;

    /// 更新进度条的消息
    ///
    /// # 参数
    /// - `id`: 进度条 ID
    /// - `message`: 新的消息
    fn update_message(&self, id: ProgressId, message: String);

    /// 增加进度（仅对 Bar 类型有效）
    ///
    /// # 参数
    /// - `id`: 进度条 ID
    /// - `delta`: 增加的进度量
    fn increment(&self, id: ProgressId, delta: u64);

    /// 完成进度条
    ///
    /// # 参数
    /// - `id`: 进度条 ID
    /// - `final_message`: 最终消息
    fn finish(&self, id: ProgressId, final_message: String);
}

/// 进度条 ID 类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProgressId(pub u64);

/// 默认的空进度回调实现
///
/// 这个实现不会产生任何输出，适用于不需要进度显示的场景
#[derive(Debug, Default)]
pub struct NoOpProgressCallback;

impl ProgressCallback for NoOpProgressCallback {
    fn start(&self, _info: ProgressInfo) -> ProgressId {
        ProgressId(0)
    }

    fn update_message(&self, _id: ProgressId, _message: String) {}

    fn increment(&self, _id: ProgressId, _delta: u64) {}

    fn finish(&self, _id: ProgressId, _final_message: String) {}
}

/// 进度回调的包装器，便于使用
pub type ProgressCallbackArc = Arc<dyn ProgressCallback>;

/// 创建默认的空进度回调
pub fn no_op_progress_callback() -> ProgressCallbackArc {
    Arc::new(NoOpProgressCallback)
}

/// 进度条助手结构体
///
/// 提供便捷的方法来创建和管理进度条
pub struct ProgressHelper {
    callback: ProgressCallbackArc,
    step_counter: Arc<std::sync::atomic::AtomicI32>,
}

impl ProgressHelper {
    /// 创建新的进度助手，从指定的初始步骤开始
    pub fn new(callback: ProgressCallbackArc, initial_step: i32) -> Self {
        Self {
            callback,
            step_counter: Arc::new(std::sync::atomic::AtomicI32::new(initial_step)),
        }
    }

    /// 获取下一个步骤号并递增计数器
    fn next_step(&self) -> i32 {
        self.step_counter
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }

    /// 创建一个旋转进度条
    pub fn create_spinner(&self, message: impl Into<String>) -> ProgressHandler {
        let step = self.next_step();
        let info = ProgressInfo {
            progress_type: ProgressType::Spinner,
            prefix: format!("0x{:02X}", step),
            message: message.into(),
            current: None,
        };
        let id = self.callback.start(info);
        ProgressHandler {
            callback: Arc::clone(&self.callback),
            id,
        }
    }

    /// 创建一个条形进度条
    pub fn create_bar(&self, total: u64, message: impl Into<String>) -> ProgressHandler {
        let step = self.next_step();
        let info = ProgressInfo {
            progress_type: ProgressType::Bar { total },
            prefix: format!("0x{:02X}", step),
            message: message.into(),
            current: Some(0),
        };
        let id = self.callback.start(info);
        ProgressHandler {
            callback: Arc::clone(&self.callback),
            id,
        }
    }

    /// 获取当前步骤号（不递增）
    pub fn current_step(&self) -> i32 {
        self.step_counter.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// 同步步骤计数器到外部计数器
    /// 这个方法用于将内部计数器的值同步到工具的 step 字段
    pub fn sync_step_to_external(&self, external_step: &mut i32) {
        *external_step = self.current_step();
    }
}

/// 进度条处理器
///
/// 用于操作单个进度条实例
pub struct ProgressHandler {
    callback: ProgressCallbackArc,
    id: ProgressId,
}

impl ProgressHandler {
    /// 更新消息
    pub fn set_message(&self, message: impl Into<String>) {
        self.callback.update_message(self.id, message.into());
    }

    /// 增加进度
    pub fn inc(&self, delta: u64) {
        self.callback.increment(self.id, delta);
    }

    /// 完成进度条
    pub fn finish_with_message(self, message: impl Into<String>) {
        self.callback.finish(self.id, message.into());
    }
}
