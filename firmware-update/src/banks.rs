use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use regex::Regex;
use sys_mount::{Mount, Unmount, UnmountDrop, UnmountFlags};

#[derive(Copy, Clone)]
pub enum Bank { A, B }

impl Bank {
    pub fn other(&self) -> Bank
    {
        match self {
            Bank::A => Bank::B,
            Bank::B => Bank::A,
        }
    }

    pub fn device(&self) -> &'static str
    {
        match self {
            Bank::A => "/dev/mmcblk0p2",
            Bank::B => "/dev/mmcblk0p3",
        }
    }
}

impl std::fmt::Display for Bank {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Bank::A => write!(f, "A"),
            Bank::B => write!(f, "B"),
        }
    }
}

pub fn detect() -> Result<Bank, Box<dyn std::error::Error>> {
    let pattern = Regex::new(r"/dev/mmcblk0p([23])[ ]+/[ ]+ext4").unwrap();

    // Read /etc/fstab,
    // grep for line `/dev/mmcblk0p[23] / ext4 defaults,noatime 0 1`
    // and see on which partition this is

    let file = File::open("/etc/fstab")?;
    let reader = BufReader::new(file);

    for line in reader.lines() {
        let line = line?;
        if let Some(group) = pattern.captures(&line) {
            let part_number = u8::from_str_radix(&group[1], 10)?;
            return match part_number {
                2 => Ok(Bank::A),
                3 => Ok(Bank::B),
                _ => Err(format!("Partition num {} is invalid", part_number).into()),
            }
        }
    }

    Err("Could not identify bank".to_owned().into())
}

/// Format the other bank as ext4
pub fn format_other_bank() -> Result<(), Box<dyn std::error::Error>> {
    let other_bank = detect()?
        .other();

    eprintln!("Formatting {} as ext4", other_bank.device());

    let output = std::process::Command::new("mkfs.ext4")
        .arg(other_bank.device())
        .output()?;

    eprintln!("mkfs.ext4: {}", String::from_utf8_lossy(&output.stdout));
    Ok(())
}

/// Mount the other bank and return a guard that will unmount on drop.
pub fn mount_other_bank() -> Result<(Bank, UnmountDrop<Mount>), Box<dyn std::error::Error>> {
    let other_bank = detect()?
        .other();

    let mount_guard = Mount::builder()
        .fstype("ext4")
        .mount(other_bank.device(), "/mnt/other_bank")?;

    Ok((other_bank, mount_guard.into_unmount_drop(UnmountFlags::DETACH)))
}

pub fn render_fstab(bank: Bank, fstab_location: &Path) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("Regenerate fstab");
    let template_a = concat!(
        "proc            /proc           proc    defaults          0       0\n",
        "/dev/mmcblk0p1  /boot           vfat    defaults          0       2\n",
        "/dev/mmcblk0p3  /mnt/other_bank ext4    noauto,noatime    0       0\n",
        "/dev/mmcblk0p2  /               ext4    defaults,noatime  0       1\n");

    let template_b = concat!(
        "proc            /proc           proc    defaults          0       0\n",
        "/dev/mmcblk0p1  /boot           vfat    defaults          0       2\n",
        "/dev/mmcblk0p2  /mnt/other_bank ext4    noauto,noatime    0       0\n",
        "/dev/mmcblk0p3  /               ext4    defaults,noatime  0       1\n");

    let new_fstab = match bank {
        Bank::A => template_a,
        Bank::B => template_b,
    };

    let mut file = File::options()
        .write(true)
        .truncate(true)
        .open(fstab_location)?;
    file.write_all(new_fstab.as_bytes())?;
    Ok(())
}

pub fn copy_config(other_bank_root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("Copy config");

    let file = File::open("firmware-update-filelist.txt")?;
    let reader = BufReader::new(file);
    for line in reader.lines() {
        let from = PathBuf::from("/").join(line?);
        let to = other_bank_root.join(&from);
        eprintln!("Copy {} to {}", from.to_string_lossy(), to.to_string_lossy());
        std::fs::copy(from, to)?;
    }

    Ok(())
}
