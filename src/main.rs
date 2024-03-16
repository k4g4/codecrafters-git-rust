use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// spit - a simple clone of git
#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Subcommands,
}

#[derive(Subcommand)]
enum Subcommands {
    /// Initialize an empty repository
    Init { path: Option<PathBuf> },

    /// Print the contents of a blob object
    CatFile {
        #[arg(short = 'p')]
        blob_sha: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Subcommands::Init { path } => commands::init(path.unwrap_or_else(|| ".".into())),
        Subcommands::CatFile { blob_sha } => commands::cat_file(&blob_sha),
    }
}

mod commands {
    use anyhow::{bail, ensure, Context, Result};
    use flate2::read::ZlibDecoder;
    use nom::{
        bytes::complete::tag,
        character::complete::{char, digit1},
    };
    use std::{
        fs,
        io::{self, Read, Write},
        path::Path,
    };

    const DOT_GIT: &str = ".git";
    const OBJECTS: &str = "objects";
    const REFS: &str = "refs";
    const HEAD: &str = "HEAD";

    /// Initializes a new git repository by creating the .git directory and its subdirectories
    pub fn init(path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref().join(DOT_GIT);

        fs::create_dir(&path)
            .and_then(|_| fs::create_dir(path.join(OBJECTS)))
            .and_then(|_| fs::create_dir(path.join(REFS)))
            .and_then(|_| fs::write(path.join(HEAD), "ref: refs/heads/main\n"))
            .with_context(|| format!("failed to initialize {}", path.display()))?;

        println!("Initialized git directory");

        Ok(())
    }

    /// Prints the contents of a blob object if it exists in .git
    pub fn cat_file(blob_sha: &str) -> Result<()> {
        let failed_context = || format!("failed to find {blob_sha}");

        let entries = fs::read_dir(Path::new(DOT_GIT).join(OBJECTS))?;

        let entry = entries
            .filter_map(Result::ok)
            .find(|entry| {
                blob_sha
                    .get(..2)
                    .is_some_and(|sha_dir| sha_dir == entry.file_name())
            })
            .with_context(failed_context)?;

        let entries = fs::read_dir(entry.path())?;

        let entry = entries
            .filter_map(Result::ok)
            .find(|entry| {
                blob_sha
                    .get(2..)
                    .is_some_and(|sha_file| sha_file == entry.file_name())
            })
            .with_context(failed_context)?;

        let mut blob = vec![];
        ZlibDecoder::new(fs::File::open(entry.path())?).read_to_end(&mut blob)?;

        let contents = parse_blob(blob.as_slice()).context("failed to parse object file")?;
        io::stdout().write(contents)?;

        Ok(())
    }

    /// Blob object contents parsed using nom
    fn parse_blob(blob: &[u8]) -> Result<&[u8]> {
        let Ok((blob, _)) = tag::<_, _, ()>(b"blob ")(blob) else {
            bail!("object file is not a blob")
        };
        let Ok((blob, size)) = digit1::<_, ()>(blob) else {
            bail!("invalid blob size in object file")
        };
        let size = std::str::from_utf8(size)
            .context("invalid blob size in object file")?
            .parse::<usize>()
            .context("failed to parse blob size")?;
        let Ok((blob, _)) = char::<_, ()>('\0')(blob) else {
            bail!("unexpected character in object file")
        };
        ensure!(blob.len() == size, "blob size is incorrect");

        Ok(blob)
    }
}

#[cfg(test)]
mod tests {
    use super::commands;
    use std::{
        fs,
        ops::Add,
        path::{Path, PathBuf},
    };

    const TEST_DIR: &str = "test_dir";

    struct TestDir;
    impl TestDir {
        fn with() -> Self {
            let _ = fs::remove_dir_all(TEST_DIR);
            fs::create_dir(TEST_DIR).unwrap();
            Self
        }
    }
    impl AsRef<Path> for TestDir {
        fn as_ref(&self) -> &Path {
            Path::new(TEST_DIR)
        }
    }
    impl Add<&str> for &TestDir {
        type Output = PathBuf;
        fn add(self, path: &str) -> Self::Output {
            self.as_ref().join(path)
        }
    }
    impl Drop for TestDir {
        fn drop(&mut self) {
            fs::remove_dir_all(TEST_DIR).unwrap();
        }
    }

    #[test]
    fn init() {
        let test_dir = TestDir::with();

        commands::init(&test_dir).unwrap();

        let mut filenames = fs::read_dir(&test_dir + ".git")
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect::<Vec<_>>();
        filenames.sort();
        assert_eq!(filenames, ["HEAD", "objects", "refs"]);

        assert_eq!(
            fs::read(&test_dir + ".git/HEAD").unwrap(),
            b"ref: refs/heads/main\n"
        );
    }
}