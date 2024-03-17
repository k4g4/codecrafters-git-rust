use anyhow::Result;
use clap::{Parser, Subcommand};

mod commands;

/// spit - a simple clone of git
#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Subcommands,
}

#[derive(Subcommand)]
enum Subcommands {
    /// Initialize an empty repository
    Init(commands::InitArgs),

    /// Print the contents of a blob object
    CatFile(commands::CatFileArgs),

    /// Compute SHA hash of an object
    HashObject(commands::HashObjectArgs),
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Subcommands::Init(commands::InitArgs { path }) => {
            commands::init(path.unwrap_or_else(|| ".".into()))
        }

        Subcommands::CatFile(commands::CatFileArgs { blob_sha }) => commands::cat_file(&blob_sha),

        Subcommands::HashObject(commands::HashObjectArgs { write, path }) => {
            commands::hash_object(path, write)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::commands;
    use std::{
        env, fs,
        path::PathBuf,
        sync::{Mutex, MutexGuard},
    };

    const TEST_DIR: &'static str = "test_dir";
    static FORCE_SINGLE_THREAD: Mutex<()> = Mutex::new(());

    struct Setup(MutexGuard<'static, ()>);
    impl Setup {
        fn init() -> Self {
            let guard = FORCE_SINGLE_THREAD.lock().unwrap();
            let _ = fs::remove_dir_all(TEST_DIR);
            fs::create_dir(TEST_DIR).unwrap();
            env::set_current_dir(TEST_DIR).unwrap();
            Self(guard)
        }
    }
    impl Drop for Setup {
        fn drop(&mut self) {
            env::set_current_dir("..").unwrap();
            let _ = fs::remove_dir_all(TEST_DIR);
        }
    }

    #[test]
    fn init() {
        let _setup = Setup::init();

        commands::init(".").unwrap();

        let mut filenames = fs::read_dir(".git")
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect::<Vec<_>>();
        filenames.sort();
        assert_eq!(filenames, ["HEAD", "objects", "refs"]);

        assert_eq!(
            fs::read(PathBuf::from(".git").join("HEAD")).unwrap(),
            b"ref: refs/heads/main\n"
        );
    }

    #[test]
    fn store_and_load() {
        let _setup = Setup::init();
        let file = "a.txt";

        commands::init(".").unwrap();

        fs::write(&file, "Hello, world").unwrap();

        commands::hash_object(&file, true).unwrap();

        assert!(fs::metadata(
            PathBuf::from(".git")
                .join("objects")
                .join("db")
                .join("e9dba55ea8fd4d5be3868b015e044be0848ec5")
        )
        .unwrap()
        .is_file());
    }
}
