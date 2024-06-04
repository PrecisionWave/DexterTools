use std::{fs::File, io::Write, process::Command};
use regex::Regex;

use crate::DesiredBank;

pub fn get_uboot_desired_bank() -> Result<DesiredBank, Box<dyn std::error::Error>> {
    let out = Command::new("fw_printenv")
        .arg("desired_bank")
        .output()?;

    if out.status.success() {
        let stdout = String::from_utf8(out.stdout)?;

        let pattern = Regex::new(r"desired_bank=([AB])").unwrap();

        if let Some(group) = pattern.captures(&stdout) {
            let bank = &group[1];
            Ok(bank.try_into()?)
        }
        else {
            Err("desired_bank is not A or B!".into())
        }
    }
    else {
        std::io::stderr().write_all(&out.stderr).unwrap();
        Err("Failed to call fw_setenv!".into())
    }
}

pub fn set_uboot_desired_bank(bank: DesiredBank) -> Result<(), Box<dyn std::error::Error>> {
    let script = format!("desired_bank={}", bank);
    let script_filename = "ubootfw.script";

    let mut file = File::options()
        .create(true)
        .write(true)
        .truncate(true)
        .open(script_filename)?;
    file.write_all(script.as_bytes())?;

    let out = Command::new("fw_setenv")
        .arg("-s")
        .arg(script_filename)
        .output()?;

    if out.status.success() {
        Ok(())
    }
    else {
        std::io::stderr().write_all(&out.stderr).unwrap();
        Err("Failed to call fw_setenv!".into())
    }
}
