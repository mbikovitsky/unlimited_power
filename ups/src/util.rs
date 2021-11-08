use windows::{
    runtime::{Interface, Result},
    Devices::Custom::{
        IIOControlCode, IOControlAccessMode, IOControlBufferingMethod, IOControlCode,
    },
    Storage::Streams::{DataWriter, IBuffer},
};

pub fn slice_to_ibuffer(bytes: &[u8]) -> Result<IBuffer> {
    let writer = DataWriter::new()?;
    writer.WriteBytes(bytes)?;
    let buffer = writer.DetachBuffer()?;
    Ok(buffer)
}

pub fn ioctl_number_to_class(ioctl: u32) -> Result<IIOControlCode> {
    // https://docs.microsoft.com/en-us/windows-hardware/drivers/kernel/defining-i-o-control-codes

    let device_type = ((ioctl & 0xFFFF0000) >> 16) as u16;

    let access = (ioctl & 0xC000) >> 14;
    let access = match access {
        0 => IOControlAccessMode::Any,
        1 => IOControlAccessMode::Read,
        2 => IOControlAccessMode::Write,
        3 => IOControlAccessMode::ReadWrite,
        _ => unreachable!("There are only two bits!"),
    };

    let function = ((ioctl & 0x3FFC) >> 2) as u16;

    let method = ioctl & 3;
    let method = match method {
        0 => IOControlBufferingMethod::Buffered,
        1 => IOControlBufferingMethod::DirectInput,
        2 => IOControlBufferingMethod::DirectOutput,
        3 => IOControlBufferingMethod::Neither,
        _ => unreachable!("There are only two bits!"),
    };

    Ok(IOControlCode::CreateIOControlCode(device_type, function, access, method)?.cast()?)
}
