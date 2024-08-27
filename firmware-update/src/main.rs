use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::{fs::File, path::Path};
use std::io::{Read, Write};

use chrono::prelude::*;
use base64::prelude::*;
use serde::{Serialize, Deserialize};
use zstd::stream::Decoder;
use tar::Archive;

mod banks;
use banks::Bank;
mod ubootenv;

#[derive(Debug, Deserialize)]
#[serde(tag = "command")]
enum Command {
    /// Detect which bank we are running
    DetectBank,

    /// Format other bank, download and extract firmware, and copy config over
    Update {
        /// URL from where to get the firmware (.tar.zstd)
        from_url: String,

        /// Username for HTTP Basic Auth
        username: Option<String>,

        /// Password for HTTP Basic Auth
        password: Option<String>,
    },

    /// Format other bank
    FormatOtherBank,

    /// Copy config from current bank to other bank
    CopyConfig,

    /// Write the bank we want to boot into on next reboot into the U-BOOT env
    SetDesiredBank {
        bank: Bank,
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "status")]
enum CommandResult {
    Error { detail: String },
    CurrentState { state: DetectedBank },
    Ok { detail: String }
}

fn handle_command(command: Command, current_bank: &DetectedBank) -> CommandResult {
    match command {
        Command::DetectBank => CommandResult::CurrentState{state: current_bank.clone()},
        Command::Update { from_url, username, password } => {
            let r = match (username, password) {
                (None, None) => update(&from_url, None),
                (Some(u), Some(p)) => update(&from_url, Some(Credentials { username: u, password: p })),
                _ => Err("Specify both username and password, or neither".to_owned().into())
            };

            match r {
                Ok(()) => CommandResult::Ok{ detail : "Update started".to_owned() },
                Err(e) => CommandResult::Error{ detail : e.to_string() },
            }
        },
        Command::FormatOtherBank => {
            match banks::format_other_bank() {
                Ok(()) => CommandResult::Ok{ detail : "Other bank formatted".to_owned() },
                Err(e) => CommandResult::Error{ detail : e.to_string() },
            }
        },
        Command::CopyConfig => {
            match copy_config() {
                Ok(b) => CommandResult::Ok{ detail : format!("Config copied to bank {}", b) },
                Err(e) => CommandResult::Error{ detail : e.to_string() },
            }
        },
        Command::SetDesiredBank { bank } => {
            match ubootenv::set_uboot_desired_bank(bank) {
                Ok(()) => CommandResult::Ok{ detail : format!("Configured to boot bank {}", bank) },
                Err(e) => CommandResult::Error{ detail : e.to_string() },
            }
        },
    }
}

fn read_file_contents(file: &Path) -> Option<String> {
    let mut version = String::new();
    match File::open(file)
        .and_then(|mut f| f.read_to_string(&mut version))
        {
            Ok(_) => Some(version.trim().to_owned()),
            Err(e) => {
                eprintln!("Failed to read our bank version: {}", e);
                None
            }
        }
}

const VERSION_FILENAME : &'static str = "image_built_at.txt";
const EXTRACTED_AT_FILENAME : &'static str = "extracted_at.txt";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let current_bank = detect_bank().expect("detect bank");

    let ctx = zmq::Context::new();
    let socket = ctx.socket(zmq::REP).unwrap();
    socket.bind("tcp://127.0.0.1:5556").unwrap();

    let mut msg = zmq::Message::new();
    loop {
        socket.recv(&mut msg, 0).unwrap();
        let msgstr = msg.as_str().unwrap();
        println!("Received command {}", msgstr);
        let response = match serde_json::from_str::<Command>(&msgstr) {
            Ok(c) => {
                handle_command(c, &current_bank)
            }
            Err(e) => {
                eprintln!("Error parsing that command: {:?}", e);
                CommandResult::Error{ detail : e.to_string() }
            }
        };

        let responsestr = serde_json::to_string(&response).expect("serialize to JSON");
        socket.send(&responsestr, 0).expect("send ZMQ");

        std::thread::sleep(Duration::from_millis(200));
    }
}

#[derive(Serialize, Debug, Clone)]
struct DetectedBank {
    pub our_bank : banks::Bank,
    pub desired_bank : Option<banks::Bank>,

    pub our_version : String,
    pub our_extract_time : String,

    pub other_version : Option<String>,
    pub other_extract_time : Option<String>,
}

fn detect_bank() -> Result<DetectedBank, Box<dyn std::error::Error>> {
    let desired_bank = match ubootenv::get_uboot_desired_bank() {
        Ok(b) => {
            eprintln!("Desired bank from u-boot env: {}", b);
            Some(b)
        },
        Err(e) => {
            eprintln!("Failed to read Desired bank from u-boot env: {}", e);
            None
        }
    };

    let (other_bank, mount_guard) = banks::mount_other_bank()?;
    let our_bank = other_bank.other();
    let other_bank_root = mount_guard.target_path();

    let our_version = read_file_contents(&PathBuf::from("/").join(VERSION_FILENAME))
        .or(Some("N/A".to_owned()))
        .unwrap();
    let our_extract_time = read_file_contents(&PathBuf::from("/").join(EXTRACTED_AT_FILENAME))
        .or(Some("N/A".to_owned()))
        .unwrap();

    let other_version = read_file_contents(&other_bank_root.join(VERSION_FILENAME));
    let other_extract_time = read_file_contents(&other_bank_root.join(EXTRACTED_AT_FILENAME));

    Ok(DetectedBank {
        our_bank,
        desired_bank,
        our_version,
        our_extract_time,
        other_version,
        other_extract_time,
    })
}

fn copy_config() -> Result<Bank, Box<dyn std::error::Error>> {
    eprintln!("Detect and mount other bank");
    let (other_bank, mount_guard) = banks::mount_other_bank()?;
    let other_bank_root = mount_guard.target_path();
    eprintln!("Bank {} mounted to {}", other_bank, other_bank_root.to_string_lossy());
    banks::copy_config(&other_bank_root)?;
    banks::render_fstab(other_bank, &other_bank_root.join("etc/fstab"))?;
    Ok(other_bank)
}


struct Credentials {
    pub username: String,
    pub password: String,
}

fn update(url: &str, creds: Option<Credentials>) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("Starting update");

    banks::format_other_bank()?;

    eprintln!("Detect and mount other bank");
    let (other_bank, mount_guard) = banks::mount_other_bank()?;
    let other_bank_root = mount_guard.target_path();

    extract(url, creds, other_bank_root)?;

    eprintln!("Bank {} mounted to {}", other_bank, other_bank_root.to_string_lossy());

    banks::copy_config(&other_bank_root)?;

    banks::render_fstab(other_bank, &other_bank_root.join("etc/fstab"))?;

    // Dropping the mount_guard unmounts the other bank
    Ok(())
}

struct ReadWrapper {
    reader : Box<dyn Read>,
    count : Arc<AtomicUsize>,
}

impl ReadWrapper {
    pub fn new(reader: Box<dyn Read>) -> Self {
        let count = Arc::new(0.into());
        Self{ reader, count }
    }
    pub fn get_count(&self) -> Arc<AtomicUsize> { self.count.clone() }
}

impl Read for ReadWrapper {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let r = self.reader.read(buf);
        if let Ok(c) = r {
            self.count.fetch_add(c, Ordering::Relaxed);
        }
        r
    }
}

fn extract(url: &str, creds: Option<Credentials>, to_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("Setup GET request to {}", url);
    let mut request_builder = ureq::get(url)
        .timeout(std::time::Duration::from_secs(3600*6));

    if let Some(c) = creds {
        eprintln!("Add username {} HTTP Basic Auth", c.username);
        let auth_header = format!(
            "Basic {}",
            BASE64_STANDARD.encode(&format!("{}:{}", c.username, c.password))
        );

        request_builder = request_builder.set("Authorization", &auth_header);
    }

    eprintln!("Connecting");
    let response = request_builder.call()?;
    let content_length : Option<usize> = response.header("content-length")
        .and_then(|v| usize::from_str_radix(v, 10).ok());
    let mut reader = ReadWrapper::new(response.into_reader());
    let byte_counter = reader.get_count();

    eprintln!("Create zstd decoder");
    let decoder = Decoder::new(&mut reader)?;
    let mut tar_archive = Archive::new(decoder);

    eprintln!("Extract files");
    let start_time = Instant::now();
    let print_interval = Duration::from_secs(1);
    let mut next_print_time = start_time + print_interval;

    match content_length {
        Some(cl) =>
            println!("\r{}% ({}/{})  {} files extracted                 ", 0, 0, cl, 0),
        None =>
            println!("Content-Length unknown, cannot show progress"),
    }

    let mut file_count = 0;
    for entry in tar_archive.entries()? {
        let mut entry = entry?;
        if !entry.unpack_in(to_path)? {
            eprintln!("Did not unpack {}", entry.path()?.to_string_lossy());
        }
        file_count += 1;

        if let Some(cl) = content_length {
            if next_print_time < Instant::now() {
                next_print_time += print_interval;

                let bytes_transferred = byte_counter.load(Ordering::Relaxed);
                let progress_percent = bytes_transferred * 100 / cl;

                // \x33[2K is the VT100 code to clear the line
                print!("\x33[2K\r{}% ({}/{})  {} files extracted",
                    progress_percent, bytes_transferred, cl, file_count);
            }
        }
    }

    let extract_completion_time = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    eprintln!("Mark the extraction as completed at {}", extract_completion_time);

    {
        let extracted_at_path = to_path.join(EXTRACTED_AT_FILENAME);

        let mut file = File::options()
            .write(true)
            .truncate(true)
            .open(extracted_at_path)?;
        file.write_all(format!("{}\n", extract_completion_time).as_bytes())?;
    }

    eprintln!("{} files extracted", file_count);
    Ok(())
}
