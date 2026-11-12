//! Optional local implementation support, not a provisioning or identity service.
//! Applications retain their own installation, credentials and startup ownership.
#[cfg(feature = "local-auth")]
pub mod local_auth;
pub mod platform;
mod storage;
use anyhow::{Result, ensure};
pub use platform::*;
#[cfg(windows)]
pub use storage::current_user_sid;
pub use storage::{
    private_create, private_dir, private_write, private_write_data, protect_file,
    publish_directory_if_absent,
};

/// Fixed application-owned startup profile. No remote command surface is exposed.
#[derive(Clone, Copy, Debug)]
pub struct Application {
    pub display_name: &'static str,
    pub startup_description: &'static str,
    pub executable_name: &'static str,
    pub startup_prefix: &'static str,
    pub data_directory: &'static str,
    pub background_command: &'static str,
}
impl Application {
    fn validate(&self) -> Result<()> {
        for value in [
            self.executable_name,
            self.startup_prefix,
            self.data_directory,
            self.background_command,
        ] {
            ensure!(
                !value.is_empty()
                    && value
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b)),
                "Invalid application profile"
            );
        }
        ensure!(
            !self.display_name.is_empty()
                && !self.display_name.chars().any(char::is_control)
                && !self.startup_description.chars().any(char::is_control),
            "Invalid application name"
        );
        Ok(())
    }
}
pub const MACHINE_HELPER: Application = Application {
    display_name: "Constellation Computer",
    startup_description: "Constellation computer connection",
    executable_name: "constellation-machine-helper",
    startup_prefix: "constellation-machine",
    data_directory: "constellation-machine-helper",
    background_command: "run",
};
pub const OBSERVATORY: Application = Application {
    display_name: "Observatory",
    startup_description: "Observatory",
    executable_name: "agent-observatory",
    startup_prefix: "agent-observatory",
    data_directory: "agent-observatory",
    background_command: "serve",
};
