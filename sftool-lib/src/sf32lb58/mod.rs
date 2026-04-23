//! SF32LB58 芯片特定实现模块

pub mod erase_flash;
pub mod ram_command;
pub mod read_flash;
pub mod reset;
pub mod speed;
pub mod write_flash;

use crate::common::serial_io::{for_tool, sleep_with_cancel};
use crate::progress::{ProgressOperation, ProgressStatus, StubStage};
use crate::sf32lb58::ram_command::DownloadStub;
use crate::{Result, SifliTool, SifliToolBase, SifliToolTrait};
use serialport::SerialPort;
use std::time::Duration;

pub struct SF32LB58Tool {
    pub base: SifliToolBase,
    pub port: Box<dyn SerialPort>,
}

// 为 SF32LB58Tool 实现 Send 和 Sync
unsafe impl Send for SF32LB58Tool {}
unsafe impl Sync for SF32LB58Tool {}

/// DFU协议命令类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum DfuCommandType {
    ImageHeader = 1,
    ImageBody = 2,
    Config = 3,
    End = 4,
}

/// DFU配置类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum DfuConfigType {
    BootPatchSig = 10,
}

impl SF32LB58Tool {
    // DFU协议常量
    const BLOCK_SIZE: usize = 512;
    const HDR_SIZE: usize = 32 + 296;
    const CHUNK_OVERHEAD: usize = 32 + 4;

    /// 发送DFU命令的通用方法
    fn send_dfu_command(&mut self, data_len: usize, delay_ms: Option<u64>) -> Result<()> {
        let cmd = format!("dfu_recv {}\r", data_len);
        tracing::trace!("Sending DFU command: {}", cmd.trim());

        {
            let mut io = for_tool(self);
            io.write_all(cmd.as_bytes())?;
            io.flush()?;
        }

        if let Some(delay) = delay_ms {
            sleep_with_cancel(&self.base.cancel_token, Duration::from_millis(delay))?;
        }

        Ok(())
    }

    /// 发送DFU数据的通用方法
    fn send_dfu_data(&mut self, header: &[u8], data: &[u8], delay_ms: Option<u64>) -> Result<()> {
        tracing::trace!(
            "Sending DFU data: header={:?}, data_len={}",
            header,
            data.len()
        );

        {
            let mut io = for_tool(self);
            io.write_all(header)?;
            io.write_all(data)?;
            io.flush()?;
        }

        if let Some(delay) = delay_ms {
            sleep_with_cancel(&self.base.cancel_token, Duration::from_millis(delay))?;
        }

        Ok(())
    }

    fn download_stub_impl(&mut self) -> Result<()> {
        use crate::ram_stub::{self, SIG_PUB_FILE, load_stub_file};

        tracing::info!("Starting SF32LB58 stub download process");
        {
            let mut io = for_tool(self);
            io.clear(serialport::ClearBuffer::All)?;
        }

        let progress = self.progress();
        let spinner = progress.create_spinner(ProgressOperation::DownloadStub {
            stage: StubStage::Start,
        });

        // 1. 下载签名公钥文件 (58X_sig_pub.der)
        tracing::debug!("Loading signature public key file: {}", SIG_PUB_FILE);
        let sig_pub_data = ram_stub::RamStubFile::get(SIG_PUB_FILE).ok_or_else(|| {
            tracing::error!("Signature public key file not found: {}", SIG_PUB_FILE);
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "58X_sig_pub.der file not found",
            )
        })?;

        spinner.set_operation(ProgressOperation::DownloadStub {
            stage: StubStage::SignatureKey,
        });
        self.download_boot_patch_sigkey(&sig_pub_data.data)?;

        // 2. 下载RAM stub文件 - 支持外部 stub 文件
        let chip_memory_key = format!("sf32lb58_{}", self.base.memory_type);
        let stub = load_stub_file(self.base.external_stub_path.as_deref(), &chip_memory_key)?;

        spinner.set_operation(ProgressOperation::DownloadStub {
            stage: StubStage::RamStub,
        });

        // 发送下载镜像命令（flashid = 9 对应RAM stub）
        self.download_image(&stub.data, 9)?;

        spinner.finish(ProgressStatus::Success);

        tracing::info!("SF32LB58 stub download completed successfully");
        Ok(())
    }

    /// 下载引导补丁签名密钥
    fn download_boot_patch_sigkey(&mut self, sig_data: &[u8]) -> Result<()> {
        tracing::info!(
            "Starting boot patch signature key download, size: {} bytes",
            sig_data.len()
        );

        let header = [
            DfuCommandType::Config as u8,
            DfuConfigType::BootPatchSig as u8,
        ];
        let total_len = 2 + sig_data.len();

        self.send_dfu_command(total_len, Some(10))?;
        self.send_dfu_data(&header, sig_data, Some(4))?;

        tracing::debug!("Waiting for boot patch signature key response...");
        self.wait_for_ok_response(3000)?;

        tracing::info!("Boot patch signature key downloaded successfully");
        Ok(())
    }

    /// 下载镜像文件
    fn download_image(&mut self, data: &[u8], flash_id: u8) -> Result<()> {
        tracing::info!(
            "Starting image download: flash_id={}, size={} bytes",
            flash_id,
            data.len()
        );

        // 1. 发送镜像头部
        self.download_image_header(data, flash_id)?;

        // 2. 发送镜像主体
        self.download_image_body(data, flash_id)?;

        // 3. 发送结束标志
        self.download_image_end(flash_id)?;

        tracing::info!("Image download completed successfully");
        Ok(())
    }

    /// 下载镜像头部
    fn download_image_header(&mut self, data: &[u8], flash_id: u8) -> Result<()> {
        tracing::debug!("Downloading image header...");

        let header = [DfuCommandType::ImageHeader as u8, flash_id];
        let total_len = 2 + Self::HDR_SIZE;

        self.send_dfu_command(total_len, Some(10))?;
        self.send_dfu_data(&header, &data[0..Self::HDR_SIZE], None)?;

        tracing::debug!("Waiting for image header response...");
        self.wait_for_ok_response(3000)?;

        tracing::debug!("Image header downloaded successfully");
        Ok(())
    }

    /// 下载镜像主体
    fn download_image_body(&mut self, data: &[u8], flash_id: u8) -> Result<()> {
        tracing::debug!("Downloading image body...");

        let body_header = [DfuCommandType::ImageBody as u8, flash_id];
        let mut offset = Self::HDR_SIZE;
        let mut chunk_count = 0;

        while offset < data.len() {
            self.base.check_cancelled()?;
            let remaining = data.len() - offset;
            let chunk_size = std::cmp::min(remaining, Self::CHUNK_OVERHEAD + Self::BLOCK_SIZE);

            tracing::trace!(
                "Sending chunk {}: offset={}, size={}",
                chunk_count,
                offset,
                chunk_size
            );

            let total_len = 2 + chunk_size;
            self.send_dfu_command(total_len, Some(10))?;
            self.send_dfu_data(&body_header, &data[offset..offset + chunk_size], None)?;

            tracing::trace!("Waiting for chunk {} response...", chunk_count);
            self.wait_for_ok_response(3000)?;

            offset += chunk_size;
            chunk_count += 1;
        }

        tracing::debug!("Image body downloaded successfully: {} chunks", chunk_count);
        Ok(())
    }

    /// 下载镜像结束标志
    fn download_image_end(&mut self, flash_id: u8) -> Result<()> {
        tracing::debug!("Sending image end marker...");

        let end_header = [DfuCommandType::End as u8, flash_id];

        self.send_dfu_command(2, Some(10))?;
        self.send_dfu_data(&end_header, &[], None)?;

        tracing::debug!("Waiting for image end response...");
        self.wait_for_ok_response(5000)?;

        tracing::debug!("Image end marker sent successfully");
        Ok(())
    }

    /// 等待OK响应
    fn wait_for_ok_response(&mut self, timeout_ms: u64) -> Result<()> {
        tracing::trace!("Waiting for OK response with timeout: {}ms", timeout_ms);
        let matched = {
            let mut io = for_tool(self);
            io.wait_for_patterns(
                &[b"OK", b"Fail"],
                Duration::from_millis(timeout_ms),
                "OK response",
            )?
        };

        let response_str = String::from_utf8_lossy(&matched.buffer);
        if matched.index == 0 {
            tracing::trace!("Received OK response: '{}'", response_str);
            return Ok(());
        }

        tracing::error!("Received Fail response: '{}'", response_str);
        Err(std::io::Error::other(format!("Received Fail response: {}", response_str)).into())
    }
}

impl SifliTool for SF32LB58Tool {
    fn create_tool(base: SifliToolBase) -> Box<dyn SifliTool> {
        let mut port = serialport::new(&base.port_name, 1000000)
            .timeout(Duration::from_secs(5))
            .open()
            .unwrap();
        port.write_request_to_send(false).unwrap();
        std::thread::sleep(Duration::from_millis(100));

        let mut tool = Box::new(Self { base, port });
        if tool.base.before.should_download_stub() {
            tool.download_stub().expect("Failed to download stub");
        }
        tool
    }
}

impl SifliToolTrait for SF32LB58Tool {
    fn port(&mut self) -> &mut Box<dyn SerialPort> {
        &mut self.port
    }

    fn base(&self) -> &SifliToolBase {
        &self.base
    }

    fn set_speed(&mut self, _baud: u32) -> Result<()> {
        todo!("SF32LB58Tool::set_speed not implemented yet")
    }

    fn soft_reset(&mut self) -> Result<()> {
        use crate::reset::Reset;
        Reset::soft_reset(self)
    }
}
