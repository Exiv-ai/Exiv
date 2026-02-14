use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::info;

#[derive(Parser)]
#[command(
    name = "vers_system",
    version = env!("CARGO_PKG_VERSION"),
    about = "VERS System - Versatile Event-driven Reasoning System"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Install VERS to a directory (self-install)
    Install {
        /// Installation directory
        #[arg(long, default_value_os_t = default_prefix())]
        prefix: PathBuf,
        /// Register as OS service (systemd on Linux, sc.exe on Windows)
        #[arg(long)]
        service: bool,
        /// Skip Python virtual environment setup
        #[arg(long)]
        no_python: bool,
        /// Service user (Linux only, default: current user)
        #[arg(long)]
        user: Option<String>,
    },
    /// Uninstall VERS
    Uninstall {
        /// Installation directory to remove
        #[arg(long, default_value_os_t = default_prefix())]
        prefix: PathBuf,
    },
    /// Manage OS service
    Service {
        #[command(subcommand)]
        action: ServiceAction,
    },
    /// Print version and build information
    Version,
    /// Internal: perform exe swap after parent exits (used by update mechanism)
    #[command(hide = true)]
    SwapExe {
        #[arg(long)]
        target: PathBuf,
        #[arg(long)]
        pid: u32,
    },
}

#[derive(Subcommand)]
pub enum ServiceAction {
    /// Register VERS as an OS service
    Install {
        #[arg(long, default_value_os_t = default_prefix())]
        prefix: PathBuf,
        #[arg(long)]
        user: Option<String>,
    },
    /// Remove VERS OS service
    Uninstall,
    /// Start the VERS service
    Start,
    /// Stop the VERS service
    Stop,
    /// Show VERS service status
    Status,
}

fn default_prefix() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(r"C:\ProgramData\VERS")
    } else {
        PathBuf::from("/opt/vers")
    }
}

/// Dispatch CLI subcommands
pub async fn dispatch(cmd: Commands) -> anyhow::Result<()> {
    match cmd {
        Commands::Install { prefix, service, no_python, user } => {
            info!("📦 Installing VERS to {}", prefix.display());
            crate::installer::install(prefix, service, no_python, user).await
        }
        Commands::Uninstall { prefix } => {
            info!("🗑️  Uninstalling VERS from {}", prefix.display());
            crate::installer::uninstall(prefix).await
        }
        Commands::Service { action } => {
            match action {
                ServiceAction::Install { prefix, user } => {
                    crate::platform::install_service(&prefix, user.as_deref())
                }
                ServiceAction::Uninstall => crate::platform::uninstall_service(),
                ServiceAction::Start => crate::platform::start_service(),
                ServiceAction::Stop => crate::platform::stop_service(),
                ServiceAction::Status => {
                    let status = crate::platform::service_status()?;
                    println!("{}", status);
                    Ok(())
                }
            }
        }
        Commands::Version => {
            println!("VERS System v{}", env!("CARGO_PKG_VERSION"));
            println!("Build target: {}", env!("TARGET"));
            Ok(())
        }
        Commands::SwapExe { target, pid } => {
            crate::platform::execute_swap(target, pid)
        }
    }
}
