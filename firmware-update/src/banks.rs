use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use serde::{Serialize, Deserialize};
use regex::Regex;
use sys_mount::{Mount, Unmount, UnmountDrop, UnmountFlags};

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
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

impl From<&str> for Bank {
    fn from(value: &str) -> Self {
        match value {
            "A" => Bank::A,
            "B" => Bank::B,
            _ => panic!("Valid banks: A or B"),
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
        .arg("-L")
        .arg(match other_bank {
            Bank::A => "bank_a",
            Bank::B => "bank_b",
        })
        .arg(other_bank.device())
        .output()?;

    eprintln!("mkfs.ext4: {}", String::from_utf8_lossy(&output.stdout));
    Ok(())
}

pub struct MountGuard {
    pub other_bank : Bank,
    pub guard : UnmountDrop<Mount>
}

/// Mount the other bank and return a guard that will unmount on drop.
pub fn mount_other_bank() -> Result<MountGuard, Box<dyn std::error::Error>> {
    let other_bank = detect()?
        .other();

    let other_bank_mountpoint = "/mnt/other_bank";

    if !Path::new(other_bank_mountpoint).is_dir() {
        if let Err(e) = std::fs::create_dir(other_bank_mountpoint) {
            eprintln!("Cannot create dir {}: {}", other_bank_mountpoint, e);
        }
        eprintln!("Created {}", other_bank_mountpoint);
    }

    let mount_guard = Mount::builder()
        .fstype("ext4")
        .mount(other_bank.device(), other_bank_mountpoint)?;

    Ok(MountGuard{
        other_bank,
        guard: mount_guard.into_unmount_drop(UnmountFlags::DETACH)
    })
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
        let filename = line?;
        let from = PathBuf::from("/").join(&filename);
        let to = other_bank_root.join(&filename);
        if from != to {
            eprintln!("Copy {} to {}", from.to_string_lossy(), to.to_string_lossy());
            std::fs::copy(from, to)?;
        }
        else {
            eprintln!("Refusing to copy {} to {}!", from.to_string_lossy(), to.to_string_lossy());
        }
    }

    Ok(())
}
