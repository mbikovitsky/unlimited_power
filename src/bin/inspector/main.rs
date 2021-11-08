use std::error::Error;

use clap::{arg_enum, crate_authors, crate_name, crate_version, value_t, App, Arg, SubCommand};

use ups::{
    hid_device::HidDevice, megatec_hid_ups::MegatecHidUps, ups::Ups,
    voltronic_hid_ups::VoltronicHidUps,
};

arg_enum! {
    #[derive(Debug, PartialEq, Eq)]
    pub enum Model {
        Voltronic,
        Megatec,
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = App::new(crate_name!())
        .version(crate_version!())
        .author(crate_authors!())
        .about("Inspects a connected UPS")
        .arg(
            Arg::with_name("model")
                .short("m")
                .long("model")
                .required(true)
                .takes_value(true)
                .case_insensitive(true)
                .possible_values(&Model::variants())
                .help("The UPS model"),
        )
        .arg(
            Arg::with_name("vid")
                .short("v")
                .long("vid")
                .required(true)
                .takes_value(true)
                .help("The VID of the UPS"),
        )
        .arg(
            Arg::with_name("pid")
                .short("p")
                .long("pid")
                .required(true)
                .takes_value(true)
                .help("The PID of the UPS"),
        )
        .arg(
            Arg::with_name("usage_id")
                .short("U")
                .long("usage-id")
                .takes_value(true)
                .help("The HID usage ID of the UPS"),
        )
        .arg(
            Arg::with_name("usage_page")
                .short("P")
                .long("usage-page")
                .takes_value(true)
                .help("The HID usage page of the UPS"),
        )
        .subcommand(SubCommand::with_name("status").about("Displays the UPS status"))
        .get_matches();

    let model = value_t!(args, "model", Model)?;
    let vendor_id = args.value_of("vid").unwrap().parse()?;
    let product_id = args.value_of("pid").unwrap().parse()?;
    let usage_page = args.value_of("usage_page").map(str::parse).transpose()?;
    let usage_id = args.value_of("usage_id").map(str::parse).transpose()?;

    let device = HidDevice::new(usage_page, usage_id, vendor_id, product_id).await?;

    let ups: Box<dyn Ups> = match model {
        Model::Voltronic => Box::new(VoltronicHidUps::new(device)?),
        Model::Megatec => Box::new(MegatecHidUps::new(device)?),
    };

    match args.subcommand() {
        ("status", _) => {
            let status = ups.status().await?;
            println!("{:#?}", status);
        }
        ("", None) => {
            eprintln!("A subcommand must be specified");
        }
        _ => unreachable!(),
    }

    Ok(())
}
