use crate::{CancelToken, Error, Result, SifliToolTrait};
use serialport::{ClearBuffer, SerialPort};
use std::collections::VecDeque;
use std::io::{self, ErrorKind, Read, Write};
use std::time::{Duration, Instant};

#[cfg(test)]
use serialport::{DataBits, FlowControl, Parity, StopBits};
#[cfg(test)]
use std::sync::{Arc, Mutex};

const SLEEP_CHUNK: Duration = Duration::from_millis(25);
const IDLE_BACKOFF: Duration = Duration::from_millis(5);
const MAX_CAPTURE_BUFFER: usize = 1024;

pub struct PatternMatch {
    pub index: usize,
    pub buffer: Vec<u8>,
}

pub fn sleep_with_cancel(cancel_token: &CancelToken, duration: Duration) -> Result<()> {
    let mut remaining = duration;
    while remaining > Duration::ZERO {
        cancel_token.check_cancelled()?;
        let sleep_for = remaining.min(SLEEP_CHUNK);
        std::thread::sleep(sleep_for);
        remaining = remaining.saturating_sub(sleep_for);
    }
    cancel_token.check_cancelled()
}

pub fn io_cancelled_error() -> io::Error {
    io::Error::new(ErrorKind::Interrupted, Error::Cancelled)
}

pub fn is_cancelled_io_error(error: &io::Error) -> bool {
    if error.kind() != ErrorKind::Interrupted {
        return false;
    }

    error
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<Error>())
        .is_some_and(|inner| matches!(inner, Error::Cancelled))
}

pub struct CancelableReader {
    port: Box<dyn SerialPort>,
    cancel_token: CancelToken,
}

impl Read for CancelableReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.cancel_token
            .check_cancelled()
            .map_err(|_| io_cancelled_error())?;
        self.port.read(buf)
    }
}

pub struct CancelableWriter {
    port: Box<dyn SerialPort>,
    cancel_token: CancelToken,
}

impl Write for CancelableWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.cancel_token
            .check_cancelled()
            .map_err(|_| io_cancelled_error())?;
        self.port.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.cancel_token
            .check_cancelled()
            .map_err(|_| io_cancelled_error())?;
        self.port.flush()
    }
}

pub struct SerialIo<'a> {
    port: &'a mut dyn SerialPort,
    cancel_token: CancelToken,
}

impl<'a> SerialIo<'a> {
    pub fn new(port: &'a mut dyn SerialPort, cancel_token: CancelToken) -> Self {
        Self { port, cancel_token }
    }

    pub fn cancel_token(&self) -> &CancelToken {
        &self.cancel_token
    }

    pub fn check_cancelled(&self) -> Result<()> {
        self.cancel_token.check_cancelled()
    }

    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.check_cancelled()?;
        self.port.read(buf).map_err(Into::into)
    }

    pub fn write_all(&mut self, buf: &[u8]) -> Result<()> {
        self.check_cancelled()?;
        self.port.write_all(buf)?;
        self.check_cancelled()
    }

    pub fn flush(&mut self) -> Result<()> {
        self.check_cancelled()?;
        self.port.flush()?;
        self.check_cancelled()
    }

    pub fn clear(&mut self, buffer: ClearBuffer) -> Result<()> {
        self.check_cancelled()?;
        self.port.clear(buffer)?;
        self.check_cancelled()
    }

    pub fn set_baud_rate(&mut self, baud_rate: u32) -> Result<()> {
        self.check_cancelled()?;
        self.port.set_baud_rate(baud_rate)?;
        self.check_cancelled()
    }

    pub fn write_request_to_send(&mut self, level: bool) -> Result<()> {
        self.check_cancelled()?;
        self.port.write_request_to_send(level)?;
        self.check_cancelled()
    }

    pub fn sleep(&self, duration: Duration) -> Result<()> {
        sleep_with_cancel(&self.cancel_token, duration)
    }

    pub fn try_clone_reader(&mut self) -> Result<CancelableReader> {
        self.check_cancelled()?;
        Ok(CancelableReader {
            port: self.port.try_clone()?,
            cancel_token: self.cancel_token.clone(),
        })
    }

    pub fn try_clone_writer(&mut self) -> Result<CancelableWriter> {
        self.check_cancelled()?;
        Ok(CancelableWriter {
            port: self.port.try_clone()?,
            cancel_token: self.cancel_token.clone(),
        })
    }

    pub fn read_exact_with_timeout(
        &mut self,
        buf: &mut [u8],
        timeout: Duration,
        context: &str,
    ) -> Result<()> {
        if buf.is_empty() {
            return Ok(());
        }

        let mut last_activity = Instant::now();
        let mut offset = 0usize;

        while offset < buf.len() {
            self.check_cancelled()?;
            match self.port.read(&mut buf[offset..]) {
                Ok(0) => {
                    if last_activity.elapsed() > timeout {
                        return Err(Error::timeout(format!("waiting for {}", context)));
                    }
                    self.sleep(IDLE_BACKOFF)?;
                }
                Ok(n) => {
                    offset += n;
                    last_activity = Instant::now();
                }
                Err(error)
                    if matches!(error.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock) =>
                {
                    if last_activity.elapsed() > timeout {
                        return Err(Error::timeout(format!("waiting for {}", context)));
                    }
                    self.sleep(IDLE_BACKOFF)?;
                }
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(error) => return Err(error.into()),
            }
        }

        Ok(())
    }

    pub fn read_line_with_timeout(&mut self, timeout: Duration, context: &str) -> Result<String> {
        let mut buffer = Vec::new();
        let mut last_activity = Instant::now();

        loop {
            self.check_cancelled()?;
            let mut byte = [0u8; 1];
            match self.port.read(&mut byte) {
                Ok(0) => {
                    if last_activity.elapsed() > timeout {
                        return Err(Error::timeout(format!("waiting for {}", context)));
                    }
                }
                Ok(_) => {
                    last_activity = Instant::now();
                    match byte[0] {
                        b'\n' => break,
                        b'\r' => continue,
                        ch => buffer.push(ch),
                    }
                }
                Err(error)
                    if matches!(error.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock) =>
                {
                    if last_activity.elapsed() > timeout {
                        return Err(Error::timeout(format!("waiting for {}", context)));
                    }
                }
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(error) => return Err(error.into()),
            }
        }

        Ok(String::from_utf8_lossy(&buffer).into_owned())
    }

    pub fn read_non_empty_line_with_timeout(
        &mut self,
        timeout: Duration,
        context: &str,
    ) -> Result<String> {
        loop {
            let line = self.read_line_with_timeout(timeout, context)?;
            let trimmed = line.trim().to_string();
            if !trimmed.is_empty() {
                return Ok(trimmed);
            }
        }
    }

    pub fn wait_for_pattern(
        &mut self,
        pattern: &[u8],
        timeout: Duration,
        context: &str,
    ) -> Result<Vec<u8>> {
        let matched = self.wait_for_patterns(&[pattern], timeout, context)?;
        Ok(matched.buffer)
    }

    pub fn wait_for_patterns(
        &mut self,
        patterns: &[&[u8]],
        timeout: Duration,
        context: &str,
    ) -> Result<PatternMatch> {
        let start = Instant::now();
        let max_len = patterns
            .iter()
            .map(|pattern| pattern.len())
            .max()
            .unwrap_or(0);
        let mut buffer = Vec::new();
        let mut window = VecDeque::with_capacity(max_len.max(1));

        loop {
            self.check_cancelled()?;
            if start.elapsed() > timeout {
                return Err(Error::timeout(format!("waiting for {}", context)));
            }

            let mut byte = [0u8; 1];
            match self.port.read(&mut byte) {
                Ok(0) => continue,
                Ok(_) => {
                    buffer.push(byte[0]);
                    if buffer.len() > MAX_CAPTURE_BUFFER {
                        let drain_len = buffer.len() - MAX_CAPTURE_BUFFER;
                        buffer.drain(..drain_len);
                    }
                    window.push_back(byte[0]);
                    if window.len() > max_len {
                        window.pop_front();
                    }

                    for (index, pattern) in patterns.iter().enumerate() {
                        if window.len() >= pattern.len()
                            && window
                                .iter()
                                .rev()
                                .take(pattern.len())
                                .rev()
                                .copied()
                                .eq(pattern.iter().copied())
                        {
                            return Ok(PatternMatch { index, buffer });
                        }
                    }
                }
                Err(error)
                    if matches!(error.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock) =>
                {
                    continue;
                }
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(error) => return Err(error.into()),
            }
        }
    }

    pub fn wait_for_prompt(
        &mut self,
        prompt: &[u8],
        retry_interval: Duration,
        max_retries: u32,
    ) -> Result<()> {
        let mut retry_count = 0u32;
        let mut window = VecDeque::with_capacity(prompt.len().max(1));
        let mut last_retry = Instant::now();

        self.write_all(b"\r\n")?;
        self.flush()?;

        loop {
            self.check_cancelled()?;

            if last_retry.elapsed() > retry_interval {
                self.clear(ClearBuffer::All)?;
                self.sleep(Duration::from_millis(100))?;
                retry_count = retry_count.saturating_add(1);
                if retry_count > max_retries {
                    return Err(Error::timeout("waiting for shell prompt"));
                }
                last_retry = Instant::now();
                window.clear();
                self.write_all(b"\r\n")?;
                self.flush()?;
            }

            let mut byte = [0u8; 1];
            match self.port.read(&mut byte) {
                Ok(0) => self.sleep(IDLE_BACKOFF)?,
                Ok(_) => {
                    window.push_back(byte[0]);
                    if window.len() > prompt.len() {
                        window.pop_front();
                    }

                    if window.len() == prompt.len()
                        && window.iter().copied().eq(prompt.iter().copied())
                    {
                        return Ok(());
                    }
                }
                Err(error)
                    if matches!(error.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock) =>
                {
                    self.sleep(IDLE_BACKOFF)?;
                }
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(error) => return Err(error.into()),
            }
        }
    }
}

pub fn for_tool<T: SifliToolTrait + ?Sized>(tool: &mut T) -> SerialIo<'_> {
    let cancel_token = tool.base().cancel_token.clone();
    SerialIo::new(tool.port().as_mut(), cancel_token)
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;

    #[derive(Default)]
    pub struct TestSerialPortState {
        pub read_data: VecDeque<u8>,
        pub writes: Vec<u8>,
        pub baud_rate: u32,
        pub timeout: Duration,
        pub clear_calls: usize,
        pub rts_history: Vec<bool>,
        pub write_calls: usize,
        pub cancel_on_write_call: Option<(usize, CancelToken)>,
    }

    pub struct TestSerialPort {
        state: Arc<Mutex<TestSerialPortState>>,
    }

    impl TestSerialPort {
        pub fn from_bytes(bytes: &[u8]) -> (Self, Arc<Mutex<TestSerialPortState>>) {
            let state = Arc::new(Mutex::new(TestSerialPortState {
                read_data: bytes.iter().copied().collect(),
                baud_rate: 1_000_000,
                timeout: Duration::from_millis(5),
                ..Default::default()
            }));
            (
                Self {
                    state: state.clone(),
                },
                state,
            )
        }
    }

    impl Read for TestSerialPort {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let mut state = self.state.lock().unwrap();
            if state.read_data.is_empty() {
                return Err(io::Error::new(ErrorKind::TimedOut, "no data"));
            }

            let bytes_read = buf.len().min(state.read_data.len());
            for slot in buf.iter_mut().take(bytes_read) {
                *slot = state.read_data.pop_front().unwrap();
            }
            Ok(bytes_read)
        }
    }

    impl Write for TestSerialPort {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let mut state = self.state.lock().unwrap();
            state.write_calls = state.write_calls.saturating_add(1);
            state.writes.extend_from_slice(buf);
            if let Some((target_call, token)) = &state.cancel_on_write_call
                && state.write_calls >= *target_call
            {
                token.cancel();
            }
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl SerialPort for TestSerialPort {
        fn name(&self) -> Option<String> {
            Some("test-port".to_string())
        }

        fn baud_rate(&self) -> serialport::Result<u32> {
            Ok(self.state.lock().unwrap().baud_rate)
        }

        fn data_bits(&self) -> serialport::Result<DataBits> {
            Ok(DataBits::Eight)
        }

        fn flow_control(&self) -> serialport::Result<FlowControl> {
            Ok(FlowControl::None)
        }

        fn parity(&self) -> serialport::Result<Parity> {
            Ok(Parity::None)
        }

        fn stop_bits(&self) -> serialport::Result<StopBits> {
            Ok(StopBits::One)
        }

        fn timeout(&self) -> Duration {
            self.state.lock().unwrap().timeout
        }

        fn set_baud_rate(&mut self, baud_rate: u32) -> serialport::Result<()> {
            self.state.lock().unwrap().baud_rate = baud_rate;
            Ok(())
        }

        fn set_data_bits(&mut self, _: DataBits) -> serialport::Result<()> {
            Ok(())
        }

        fn set_flow_control(&mut self, _: FlowControl) -> serialport::Result<()> {
            Ok(())
        }

        fn set_parity(&mut self, _: Parity) -> serialport::Result<()> {
            Ok(())
        }

        fn set_stop_bits(&mut self, _: StopBits) -> serialport::Result<()> {
            Ok(())
        }

        fn set_timeout(&mut self, timeout: Duration) -> serialport::Result<()> {
            self.state.lock().unwrap().timeout = timeout;
            Ok(())
        }

        fn write_request_to_send(&mut self, level: bool) -> serialport::Result<()> {
            self.state.lock().unwrap().rts_history.push(level);
            Ok(())
        }

        fn write_data_terminal_ready(&mut self, _: bool) -> serialport::Result<()> {
            Ok(())
        }

        fn read_clear_to_send(&mut self) -> serialport::Result<bool> {
            Ok(false)
        }

        fn read_data_set_ready(&mut self) -> serialport::Result<bool> {
            Ok(false)
        }

        fn read_ring_indicator(&mut self) -> serialport::Result<bool> {
            Ok(false)
        }

        fn read_carrier_detect(&mut self) -> serialport::Result<bool> {
            Ok(false)
        }

        fn bytes_to_read(&self) -> serialport::Result<u32> {
            Ok(self.state.lock().unwrap().read_data.len() as u32)
        }

        fn bytes_to_write(&self) -> serialport::Result<u32> {
            Ok(0)
        }

        fn clear(&self, _: ClearBuffer) -> serialport::Result<()> {
            self.state.lock().unwrap().clear_calls += 1;
            Ok(())
        }

        fn try_clone(&self) -> serialport::Result<Box<dyn SerialPort>> {
            Ok(Box::new(Self {
                state: self.state.clone(),
            }))
        }

        fn set_break(&self) -> serialport::Result<()> {
            Ok(())
        }

        fn clear_break(&self) -> serialport::Result<()> {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Duration, *};
    use crate::CancelToken;

    #[test]
    fn wait_for_pattern_stops_when_cancelled() {
        let (mut port, _) = test_support::TestSerialPort::from_bytes(&[]);
        let token = CancelToken::new();
        token.cancel();
        let mut io = SerialIo::new(&mut port, token);

        let result = io.wait_for_pattern(b"OK", Duration::from_millis(50), "OK response");

        assert!(matches!(result, Err(Error::Cancelled)));
    }

    #[test]
    fn wait_for_prompt_retries_and_can_be_cancelled() {
        let (mut port, _) = test_support::TestSerialPort::from_bytes(&[]);
        let token = CancelToken::new();
        token.cancel();
        let mut io = SerialIo::new(&mut port, token);

        let result = io.wait_for_prompt(b"msh >", Duration::from_millis(50), 1);

        assert!(matches!(result, Err(Error::Cancelled)));
    }

    #[test]
    fn cloned_reader_reports_cancelled_io_error() {
        let (mut port, state) = test_support::TestSerialPort::from_bytes(b"abc");
        let token = CancelToken::new();
        state.lock().unwrap().cancel_on_write_call = Some((1, token.clone()));
        let mut io = SerialIo::new(&mut port, token);

        let mut reader = io.try_clone_reader().unwrap();
        let mut writer = io.try_clone_writer().unwrap();
        writer.write_all(b"x").unwrap();

        let mut buffer = [0u8; 1];
        let error = reader.read(&mut buffer).unwrap_err();
        assert!(is_cancelled_io_error(&error));
    }

    #[test]
    fn wait_for_patterns_bounds_captured_buffer() {
        let mut bytes = vec![b'a'; MAX_CAPTURE_BUFFER + 32];
        bytes.extend_from_slice(b"OK");
        let (mut port, _) = test_support::TestSerialPort::from_bytes(&bytes);
        let token = CancelToken::new();
        let mut io = SerialIo::new(&mut port, token);

        let matched = io
            .wait_for_patterns(&[b"OK"], Duration::from_millis(100), "OK response")
            .unwrap();

        assert!(matched.buffer.len() <= MAX_CAPTURE_BUFFER);
        assert!(matched.buffer.ends_with(b"OK"));
    }
}
