use std::error::Error;

use clap::{crate_authors, crate_name, crate_version, App, Arg, SubCommand};

use ups::hid_device::HidDevice;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = App::new(crate_name!())
        .version(crate_version!())
        .author(crate_authors!())
        .about("Inspects a connected UPS")
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

    let vendor_id = args.value_of("vid").unwrap().parse()?;
    let product_id = args.value_of("pid").unwrap().parse()?;
    let usage_page = args.value_of("usage_page").map(str::parse).transpose()?;
    let usage_id = args.value_of("usage_id").map(str::parse).transpose()?;

    let device = HidDevice::new(usage_page, usage_id, vendor_id, product_id).await?;

    match args.subcommand() {
        ("status", _) => {
            let status = device.get_indexed_string(3).await?;
            dbg!(status);
        }
        ("", None) => {
            eprintln!("A subcommand must be specified");
        }
        _ => unreachable!(),
    }

    Ok(())
}
