use std::{fs::File, io::Write, process::Command};
use regex::Regex;

use crate::banks::Bank;

pub enum UBootBankVariable {
    Desired,
    LastTried,
    LastOk,
}

impl UBootBankVariable {
    pub fn env_var_name(&self) -> &'static str {
        match self {
            UBootBankVariable::Desired => "desired_bank",
            UBootBankVariable::LastTried => "last_tried_bank",
            UBootBankVariable::LastOk => "last_ok_bank",
        }
    }
}

pub fn get_uboot_bank(bank_variable: UBootBankVariable) -> Result<Bank, Box<dyn std::error::Error>> {
    let var_name = bank_variable.env_var_name();
    let out = Command::new("fw_printenv")
        .arg(var_name)
        .output()?;

    if out.status.success() {
        let stdout = String::from_utf8(out.stdout)?;

        let pattern = Regex::new(&format!(r"{}=([AB])", var_name)).unwrap();

        if let Some(group) = pattern.captures(&stdout) {
            let bank = &group[1];
            Ok(bank.try_into()?)
        }
        else {
            Err(format!("{} is not A or B!", var_name).into())
        }
    }
    else {
        std::io::stderr().write_all(&out.stderr).unwrap();
        Err("Failed to call fw_setenv!".into())
    }
}

pub fn set_uboot_bank(var_name: UBootBankVariable, bank: Bank) -> Result<(), Box<dyn std::error::Error>> {
    let script = format!("{}={}", var_name.env_var_name(), bank);
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
