use std::path::PathBuf;
use std::{fs::File, path::Path};
use std::io::Read;

use base64::prelude::*;
use clap::{Parser, Subcommand};
use zstd::stream::Decoder;
use tar::Archive;

mod banks;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Detect which bank we are running
    DetectBank,

    /// Format other bank
    FormatOtherBank,

    /// Format other bank, download and extract firmware
    Update {
        /// URL from where to get the firmware (.tar.zstd)
        #[arg(short = 'f', long, value_name = "URL")]
        from_url: String,

        /// Username for HTTP Basic Auth
        #[arg(long)]
        username: Option<String>,

        /// Password for HTTP Basic Auth
        #[arg(long)]
        password: Option<String>,
    },


    // TODO: subcommand to set bank to boot
}

fn read_bank_version(version_file_location: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let mut file = File::open(version_file_location)?;
    let mut version = String::new();
    file.read_to_string(&mut version)?;
    Ok(version)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Update { from_url, username, password } => {
            match (username, password) {
                (None, None) => update(&from_url, None),
                (Some(u), Some(p)) => update(&from_url, Some(Credentials { username: u, password: p })),
                _ => Err("Specify both username and password, or neither".to_owned().into())
            }
        },
        Commands::DetectBank => {
            eprintln!("Detect and mount other bank");
            let (other_bank, mount_guard) = banks::mount_other_bank()?;
            let other_bank_root = mount_guard.target_path();

            let version_filename = "image_built_at.txt";

            let our_version = read_bank_version(&PathBuf::from("/").join(version_filename))?;
            eprintln!("We are running from bank {}, version {}", other_bank.other(), our_version);

            let other_version = read_bank_version(&other_bank_root.join(version_filename))?;
            eprintln!("Other bank               {}, version {}", other_bank, other_version);

            Ok(())
        },
        Commands::FormatOtherBank => {
            banks::format_other_bank()?;
            Ok(())
        }
    }
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
    let mut reader = response.into_reader();
    eprintln!("Create zstd decoder");
    let decoder = Decoder::new(&mut reader)?;
    let mut tar_archive = Archive::new(decoder);

    eprintln!("Extract files");
    let mut count = 0;
    for entry in tar_archive.entries()? {
        let mut entry = entry?;
        if !entry.unpack_in(to_path)? {
            eprintln!("Did not unpack {}", entry.path()?.to_string_lossy());
        }
        count += 1;
    }

    eprintln!("{} files extracted", count);
    Ok(())
}

