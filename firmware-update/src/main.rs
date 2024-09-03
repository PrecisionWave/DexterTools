use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{spawn, JoinHandle};
use std::time::{Duration, Instant};
use std::{fs::File, path::Path};
use std::io::{Read, Write};

use chrono::prelude::*;
use base64::prelude::*;
use serde::{Serialize, Deserialize};
use zstd::stream::Decoder;
use tar::Archive;

mod banks;
use banks::{Bank, MountGuard};
mod ubootenv;

#[derive(Debug, Deserialize)]
#[serde(tag = "command")]
enum Command {
    /// Return current status and progess
    GetStatus,

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
    Status { banks: DetectedBankInfo, progress: Option<i32> },
    Ok { detail: String }
}

const VERSION_FILENAME : &'static str = "image_built_at.txt";
const EXTRACTED_AT_FILENAME : &'static str = "extracted_at.txt";

type UpdateResult = JoinHandle<Result<MountGuard, String>>;

struct StateMachine {
    progress_state: ProgressState,
    join_handle: Option<UpdateResult>,
    bank_info_cache: DetectedBankInfo,
}

impl StateMachine {
    pub fn new() -> Self {
        let current_bank_info = banks::mount_other_bank()
            .and_then(|mg| detect_bank(&mg))
            .unwrap();

        StateMachine {
            progress_state : ProgressState::new(),
            join_handle : None,
            bank_info_cache: current_bank_info,
        }
    }

    pub fn handle_command(&mut self, command: Command) -> CommandResult {
        match command {
            Command::GetStatus => {
                self.join_handle = match self.join_handle.take() {
                    Some(j) if j.is_finished() => {
                        match j.join().expect("thread join") {
                            Ok(mg) => {
                                self.bank_info_cache = detect_bank(&mg).expect("detect bank");
                                // And dropping the mountguard will unmount the partition now
                            },
                            Err(e) => {
                                eprintln!("Update thread failed with {}", e);
                            },
                        };

                        *(self.progress_state.progress.lock().expect("lock progress state")) = None;

                        None
                    },
                    x => x,
                };

                let progress = *(self.progress_state.progress.lock().expect("lock progress state"));
                CommandResult::Status{ banks: self.bank_info_cache.clone(), progress }
            },
            Command::Update { from_url, username, password } => {
                if self.join_handle.is_some() {
                    return CommandResult::Error{ detail: "update already ongoing".to_owned() };
                }

                let r = match (username, password) {
                    (None, None) => self.update(&from_url, None),
                    (Some(u), Some(p)) => self.update(&from_url, Some(Credentials { username: u, password: p })),
                    _ => Err("Specify both username and password, or neither".to_owned().into())
                };

                match r {
                    Ok(jh) => {
                        self.join_handle = Some(jh);
                        CommandResult::Ok{ detail : "Update started".to_owned() }
                    },
                    Err(e) => CommandResult::Error{ detail : e.to_string() },
                }
            },
            Command::FormatOtherBank => {
                match banks::format_other_bank() {
                    Ok(()) => {
                        self.bank_info_cache = banks::mount_other_bank()
                            .and_then(|mg| detect_bank(&mg))
                            .unwrap();

                        CommandResult::Ok{ detail : "Other bank formatted".to_owned() }
                    },
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
                    Ok(()) => {
                        self.bank_info_cache = banks::mount_other_bank()
                            .and_then(|mg| detect_bank(&mg))
                            .unwrap();

                        CommandResult::Ok{ detail : format!("Configured to boot bank {}", bank) }
                    },
                    Err(e) => CommandResult::Error{ detail : e.to_string() },
                }
            },
        }
    }

    fn update(&mut self, url: &str, creds: Option<Credentials>) -> Result<UpdateResult, Box<dyn std::error::Error>> {
        eprintln!("Setup Firmware Update GET request to {}", url);
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
        let content_length_kb : Option<usize> = response.header("content-length")
            .and_then(|v| usize::from_str_radix(v, 10).ok())
            .map(|v| v / 1024);

        let progress_state = self.progress_state.clone();
        let thread_handle = spawn(move || {
            let f = move || -> Result<MountGuard, Box<dyn std::error::Error>> {
                progress_state.update_progress(0);

                eprintln!("Format other bank");
                banks::format_other_bank()?;

                eprintln!("Detect and mount other bank");
                let mount_guard = banks::mount_other_bank()?;
                // Dropping the mount_guard unmounts the other bank

                let other_bank_root = mount_guard.guard.target_path();

                let mut reader = ReadWrapper::new(response.into_reader());
                let kb_counter = reader.get_kilobyte_count();

                eprintln!("Create zstd decoder");
                let decoder = Decoder::new(&mut reader)?;
                let mut tar_archive = Archive::new(decoder);

                eprintln!("Extract files");
                let start_time = Instant::now();
                let print_interval = Duration::from_secs(1);
                let mut next_print_time = start_time + print_interval;

                match content_length_kb {
                    Some(cl) =>
                        eprintln!("{}% ({}/{})  {} files extracted                 ", 0, 0, cl, 0),
                    None =>
                        eprintln!("Content-Length unknown, cannot show progress"),
                }

                let mut file_count = 0;
                for entry in tar_archive.entries()? {
                    let mut entry = entry?;
                    if !entry.unpack_in(&other_bank_root)? {
                        eprintln!("Did not unpack {}", entry.path()?.to_string_lossy());
                    }
                    file_count += 1;

                    if let Some(cl) = content_length_kb {
                        if next_print_time < Instant::now() {
                            next_print_time += print_interval;

                            let kb_transferred = kb_counter.load(Ordering::Relaxed);
                            let progress_percent = kb_transferred * 100 / cl;
                            if let Ok(p) = progress_percent.try_into() {
                                progress_state.update_progress(p);
                            }

                            eprintln!("{}% ({}/{})  {} files extracted",
                            progress_percent, kb_transferred, cl, file_count);
                        }
                    }
                }

                let extract_completion_time = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
                eprintln!("Mark the extraction as completed at {}", extract_completion_time);

                {
                    let extracted_at_path = other_bank_root.join(EXTRACTED_AT_FILENAME);

                    let mut file = File::options()
                        .create_new(true)
                        .write(true)
                        .truncate(true)
                        .open(extracted_at_path)?;
                    file.write_all(format!("{}\n", extract_completion_time).as_bytes())?;
                }

                eprintln!("{} files extracted", file_count);

                eprintln!("Bank {} mounted to {}", mount_guard.other_bank, other_bank_root.to_string_lossy());
                banks::copy_config(&other_bank_root)?;
                banks::render_fstab(mount_guard.other_bank, &other_bank_root.join("etc/fstab"))?;

                eprintln!("Update completed");

                Ok(mount_guard)
            };

            match f() {
                Ok(mg) => Ok(mg),
                Err(e) => Err(format!("{:?}", e)),
            }
        });
        Ok(thread_handle)
    }
}

// State is either None: no update running; or Some(percent) when an update is running
#[derive(Clone)]
struct ProgressState {
    pub progress : Arc<Mutex<Option<i32>>>
}

impl ProgressState {
    pub fn new() -> Self {
        ProgressState { progress : Arc::new(Mutex::new(None)) }
    }

    pub fn update_progress(&self, percent: i32) -> ()
    {
        *(self.progress.lock().expect("lock mutex")) = Some(percent);
    }
}


fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut state_machine = StateMachine::new();

    let ctx = zmq::Context::new();
    let socket = ctx.socket(zmq::REP).unwrap();
    socket.bind("tcp://127.0.0.1:5552").unwrap();

    let mut msg = zmq::Message::new();
    loop {
        socket.recv(&mut msg, 0).unwrap();
        let msgstr = msg.as_str().unwrap();
        let response = match serde_json::from_str::<Command>(&msgstr) {
            Ok(c) => {
                state_machine.handle_command(c)
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
struct DetectedBankInfo {
    pub our_bank : banks::Bank,
    pub desired_bank : Option<banks::Bank>,

    pub our_version : Option<String>,
    pub our_extract_time : Option<String>,

    pub other_version : Option<String>,
    pub other_extract_time : Option<String>,
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

fn detect_bank(mount_guard: &MountGuard) -> Result<DetectedBankInfo, Box<dyn std::error::Error>> {
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

    let our_bank = mount_guard.other_bank.other();
    let other_bank_root = mount_guard.guard.target_path();

    let our_version = read_file_contents(&PathBuf::from("/").join(VERSION_FILENAME));
    let our_extract_time = read_file_contents(&PathBuf::from("/").join(EXTRACTED_AT_FILENAME));

    let other_version = read_file_contents(&other_bank_root.join(VERSION_FILENAME));
    let other_extract_time = read_file_contents(&other_bank_root.join(EXTRACTED_AT_FILENAME));

    Ok(DetectedBankInfo {
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
    let mount_guard = banks::mount_other_bank()?;
    let other_bank_root = mount_guard.guard.target_path();
    eprintln!("Bank {} mounted to {}", mount_guard.other_bank, other_bank_root.to_string_lossy());
    banks::copy_config(&other_bank_root)?;
    banks::render_fstab(mount_guard.other_bank, &other_bank_root.join("etc/fstab"))?;
    Ok(mount_guard.other_bank)
}


struct Credentials {
    pub username: String,
    pub password: String,
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
    pub fn get_kilobyte_count(&self) -> Arc<AtomicUsize> { self.count.clone() }
}

impl Read for ReadWrapper {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let r = self.reader.read(buf);
        if let Ok(c) = r {
            self.count.fetch_add(c / 1024, Ordering::Relaxed);
        }
        r
    }
}
