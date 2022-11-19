use std::error::Error;

use clap::{command, Parser, Subcommand, ValueEnum};

use ups::{
    hid_device::HidDevice,
    megatec_hid_ups::MegatecHidUps,
    ups::{Ups, UpsStatusFlags},
    voltronic_hid_ups::VoltronicHidUps,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Model {
    Voltronic,
    Megatec,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum OnOff {
    On,
    Off,
}

impl From<OnOff> for bool {
    fn from(value: OnOff) -> Self {
        match value {
            OnOff::On => true,
            OnOff::Off => false,
        }
    }
}

impl From<&OnOff> for bool {
    fn from(value: &OnOff) -> Self {
        (*value).into()
    }
}

#[derive(Debug, Parser)]
#[command(author, version, about = "Inspects a connected UPS", long_about = None)]
struct Cli {
    /// The UPS model
    #[arg(short = 'm', long)]
    model: Model,

    /// The VID of the UPS
    #[arg(short = 'v', long)]
    vendor_id: u16,

    /// The PID of the UPS
    #[arg(short = 'p', long)]
    product_id: u16,

    /// The HID usage ID of the UPS
    #[arg(short = 'U', long)]
    usage_id: Option<u16>,

    /// The HID usage page of the UPS
    #[arg(short = 'P', long)]
    usage_page: Option<u16>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Displays the UPS status
    Status,

    /// Beeper control
    Beeper {
        /// Beeper state to set
        state: Option<OnOff>,
    },
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    let device =
        HidDevice::new(cli.usage_page, cli.usage_id, cli.vendor_id, cli.product_id).await?;

    let ups: Box<dyn Ups> = match cli.model {
        Model::Voltronic => Box::new(VoltronicHidUps::new(device)?),
        Model::Megatec => Box::new(MegatecHidUps::new(device)?),
    };

    match cli.command {
        Commands::Status => {
            let status = ups.status().await?;
            println!("{:#?}", status);
        }
        Commands::Beeper { state } => {
            if let Some(state) = state {
                let on: bool = state.into();
                let should_toggle = on ^ beeper_on(ups.as_ref()).await?;

                if should_toggle {
                    ups.beeper_toggle().await?;
                }
            }

            println!(
                "Beeper is {}",
                if beeper_on(ups.as_ref()).await? {
                    "ON"
                } else {
                    "OFF"
                }
            )
        }
    }

    Ok(())
}

async fn beeper_on(ups: &dyn Ups) -> Result<bool, Box<dyn Error>> {
    Ok(ups
        .status()
        .await?
        .flags
        .contains(UpsStatusFlags::BEEPER_ACTIVE))
}
