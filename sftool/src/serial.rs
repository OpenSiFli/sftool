use anyhow::{Result, anyhow, bail};

/// Convert macOS /dev/tty.* ports to /dev/cu.* ports
///
/// On macOS, /dev/tty.* ports should be avoided in favor of /dev/cu.* ports
/// This function automatically converts any /dev/tty.* path to its /dev/cu.* equivalent
fn normalize_mac_port_name(port_name: &str) -> String {
    #[cfg(target_os = "macos")]
    {
        if port_name.starts_with("/dev/tty.") {
            return port_name.replace("/dev/tty.", "/dev/cu.");
        }
    }
    port_name.to_string()
}

pub fn normalize_port_name(port_name: &str) -> String {
    normalize_mac_port_name(port_name)
}

/// Check if the specified serial port is available
///
/// # Parameters
/// * `port_name` - The name of the serial port to check
///
/// # Returns
/// * `Result<(), String>` - Returns Ok(()) if the port is available; otherwise returns an Err with error message
pub fn check_port_available(port_name: &str) -> Result<()> {
    match serialport::available_ports() {
        Ok(ports) => {
            // On macOS, only use /dev/cu.* ports, not /dev/tty.* ports
            #[cfg(target_os = "macos")]
            let filtered_ports: Vec<_> = ports
                .into_iter()
                .filter(|port| !port.port_name.starts_with("/dev/tty."))
                .collect();

            #[cfg(not(target_os = "macos"))]
            let filtered_ports: Vec<_> = ports.into_iter().collect();

            // Check if the specified port is in the available list
            if filtered_ports.iter().any(|p| p.port_name == port_name) {
                return Ok(());
            }

            // If the port doesn't exist, return an error and list all available ports
            let available_ports: Vec<String> =
                filtered_ports.iter().map(|p| p.port_name.clone()).collect();

            bail!(
                "The specified port '{}' does not exist. Available ports: {}",
                port_name,
                if available_ports.is_empty() {
                    "No available ports".to_string()
                } else {
                    available_ports.join(", ")
                }
            )
        }
        Err(e) => Err(anyhow!("Failed to get available ports list: {}", e)),
    }
}
