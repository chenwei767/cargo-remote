use simple_logger::SimpleLogger;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::process::{exit, Command, Stdio};
use structopt::StructOpt;

use log::{error, info};

#[derive(StructOpt, Debug)]
#[structopt(name = "cargo-remote", bin_name = "cargo")]
enum Opts {
    #[structopt(name = "remote")]
    Remote {
        #[structopt(short = "r", long = "remote", help = "Remote ssh build server")]
        build_server: String,

        #[structopt(
            short = "b",
            long = "build-env",
            help = "Set remote environment variables. RUST_BACKTRACE, CC, LIB, etc. ",
            default_value = "RUST_BACKTRACE=1"
        )]
        build_env: String,

        #[structopt(
            short = "d",
            long = "rustup-default",
            help = "Rustup default (stable|beta|nightly)",
            default_value = "stable"
        )]
        rustup_default: String,

        #[structopt(
            short = "e",
            long = "env",
            help = "Environment profile. default_value = /etc/profile",
            default_value = "/etc/profile"
        )]
        env: String,

        #[structopt(
            short = "c",
            long = "copy-back",
            help = "Transfer specific files or folders from that folder back to the local machine"
        )]
        copy_back: Option<Vec<PathBuf>>,

        #[structopt(
            long = "no-copy-lock",
            help = "don't transfer the Cargo.lock file back to the local machine"
        )]
        no_copy_lock: bool,

        #[structopt(
            long = "manifest-path",
            help = "Path to the manifest to execute",
            default_value = "Cargo.toml",
            parse(from_os_str)
        )]
        manifest_path: PathBuf,

        #[structopt(
            long = "base-path",
            help = "the base dir of build path",
            default_value = "~"
        )]
        base_path: PathBuf,

        #[structopt(
            long = "build-path",
            help = "Use this build_path instead of generating build_path from a hash."
        )]
        build_path: Option<PathBuf>,

        #[structopt(
            long = "transfer-hidden",
            help = "Transfer hidden files and directories to the build server"
        )]
        hidden: bool,

        #[structopt(
            long = "transfer-compress",
            help = "Compress file data during the transfer"
        )]
        compress: bool,

        #[structopt(help = "cargo command that will be executed remotely")]
        command: String,

        #[structopt(
            help = "cargo options and flags that will be applied remotely",
            name = "remote options"
        )]
        options: Vec<String>,
    },
}

fn main() {
    SimpleLogger::new().init().unwrap();

    let Opts::Remote {
        build_server,
        build_env,
        rustup_default,
        env,
        copy_back,
        no_copy_lock,
        manifest_path,
        hidden,
        build_path,
        command,
        options,
        compress,
        base_path,
    } = Opts::from_args();

    let project_dir = {
        let mut metadata_cmd = cargo_metadata::MetadataCommand::new();
        metadata_cmd.manifest_path(manifest_path).no_deps();
        let project_metadata = metadata_cmd.exec().unwrap();
        project_metadata.workspace_root
    };
    info!("Project dir: {:?}", project_dir);

    let build_path = build_path.unwrap_or_else(|| {
        // generate a unique build path by using the hashed project dir as folder on the remote machine
        let mut hasher = DefaultHasher::new();
        project_dir.hash(&mut hasher);

        // format!("{}/remote-builds/{}/", base_path, hasher.finish())
        let mut p = PathBuf::new();
        p.push(base_path);
        if p.to_string_lossy() != "~" {
            assert!(p.is_absolute(), "The base path must be absolute path.");
        }
        p.push("remote-builds");
        p.push(hasher.finish().to_string());
        p
    });

    info!("Transferring sources to build server.");
    // transfer project to build server
    let mut rsync_to = Command::new("rsync");
    rsync_to
        .arg("-a".to_owned())
        .arg("--delete")
        .arg("--info=progress2")
        .arg("--exclude")
        .arg("target")
        .arg("--exclude")
        .arg("node_modules");

    if compress {
        rsync_to.arg("--compress");
    }

    if !hidden {
        rsync_to.arg("--exclude").arg(".*");
    }

    rsync_to
        .arg("--rsync-path")
        .arg(format!(
            "mkdir -p {} && rsync",
            build_path.to_string_lossy()
        ))
        .arg(format!("{}/", project_dir.to_string_lossy()))
        .arg(format!("{}:{}", build_server, build_path.to_string_lossy()))
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .stdin(Stdio::inherit())
        .output()
        .unwrap_or_else(|e| {
            error!("Failed to transfer project to build server (error: {})", e);
            exit(-4);
        });
    info!("Build ENV: {:?}", build_env);
    info!("Environment profile: {:?}", env);
    info!("Build path: {:?}", build_path.to_string_lossy());
    let build_command = format!(
        "source {}; rustup default {}; cd {}; {} cargo {} {}",
        env,
        rustup_default,
        build_path.to_string_lossy(),
        build_env,
        command,
        options.join(" ")
    );

    info!("Starting build process. \n{}", build_command);
    let output = Command::new("ssh")
        .arg("-t")
        .arg(&build_server)
        .arg(build_command)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .stdin(Stdio::inherit())
        .output()
        .unwrap_or_else(|e| {
            error!("Failed to run cargo command remotely (error: {})", e);
            exit(-5);
        });

    if let Some(file_names) = copy_back {
		assert!(file_names.len() > 0, "need at least a file or dir");
        for file_name in file_names {
			assert!(file_name.to_string_lossy().len() > 0, "file or dir that trans back cannot be empty!");
			let mut dir = project_dir.clone();
            dir.push(file_name.clone());
			let dir = dir.parent().unwrap().as_os_str();

            // ensure dirs.
            Command::new("mkdir")
                .arg("-p")
                .arg(dir)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .stdin(Stdio::inherit())
                .output()
                .unwrap_or_else(|e| {
                    error!(
                        "Failed to create target dir on local machine (error: {})",
                        e
                    );
                    exit(-6);
                });

            info!(
                "Transferring {} back to client.",
                file_name.to_string_lossy()
            );
            let mut rsync_to = Command::new("rsync");
            if compress {
                rsync_to.arg("--compress");
            }
            rsync_to
                .arg("-a")
                .arg("-r")
                .arg("--delete")
                .arg("--info=progress2")
                .arg(format!(
                    "{}:{}/{}",
                    build_server,
                    build_path.to_string_lossy(),
                    file_name.to_string_lossy()
                ))
                .arg(format!(
                    "{}/{}",
                    project_dir.to_string_lossy(),
                    file_name.to_string_lossy()
                ))
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .stdin(Stdio::inherit())
                .output()
                .unwrap_or_else(|e| {
                    error!(
                        "Failed to transfer target back to local machine (error: {})",
                        e
                    );
                    exit(-6);
                });
        }
    }

    if !no_copy_lock {
        info!("Transferring Cargo.lock file back to client.");
        let mut rsync_to = Command::new("rsync");
        if compress {
            rsync_to.arg("--compress");
        }
        rsync_to
            .arg("-a")
            .arg("--delete")
            .arg("--info=progress2")
            .arg(format!(
                "{}:{}/Cargo.lock",
                build_server,
                build_path.to_string_lossy()
            ))
            .arg(format!("{}/Cargo.lock", project_dir.to_string_lossy()))
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .stdin(Stdio::inherit())
            .output()
            .unwrap_or_else(|e| {
                error!(
                    "Failed to transfer Cargo.lock back to local machine (error: {})",
                    e
                );
                exit(-7);
            });
    }

    if !output.status.success() {
        exit(output.status.code().unwrap_or(1))
    }
}
