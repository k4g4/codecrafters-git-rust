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
    Init {
        /// Path to use for initializing the repository
        path: Option<PathBuf>,
    },

    /// Print the contents of a blob object
    CatFile {
        /// The object's hash
        #[arg(short = 'p')]
        blob_sha: String,
    },

    /// Compute SHA hash of an object
    HashObject {
        /// Write the object to the .git database
        #[arg(short = 'w', default_value_t = false)]
        write: bool,

        /// Path to the object
        path: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Subcommands::Init { path } => commands::init(path.unwrap_or_else(|| ".".into())),
        Subcommands::CatFile { blob_sha } => commands::cat_file(&blob_sha),
        Subcommands::HashObject { write, path } => commands::hash_object(path, write),
    }
}

mod commands {
    use anyhow::{bail, ensure, Context, Result};
    use flate2::read::{ZlibDecoder, ZlibEncoder};
    use nom::{
        bytes::complete::tag,
        character::complete::{char, digit1},
    };
    use sha1::{Digest, Sha1};
    use std::{
        ffi::OsString,
        fs,
        io::{self, Read, Write},
        path::{Path, PathBuf},
    };

    const DOT_GIT: &str = ".git";
    const OBJECTS: &str = "objects";
    const REFS: &str = "refs";
    const HEAD: &str = "HEAD";

    const SHA_LEN: usize = 40;

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

        ensure!(blob_sha.len() > 3, "object hash is not long enough");
        let (sha_dir, sha_file) = blob_sha.split_at(2);

        let entries = fs::read_dir(Path::new(DOT_GIT).join(OBJECTS))?;

        let entry = entries
            .filter_map(Result::ok)
            .find(|entry| sha_dir == entry.file_name())
            .with_context(failed_context)?;

        let entries = fs::read_dir(entry.path())?;

        let entry = entries
            .filter_map(Result::ok)
            .find(|entry| {
                entry.file_name().len() == SHA_LEN - 2
                    && entry
                        .file_name()
                        .as_os_str()
                        .to_string_lossy()
                        .starts_with(sha_file)
            })
            .with_context(failed_context)?;

        // possible optimization: read up to the filesize,
        // then perform just one allocation for the next read
        let mut blob = vec![];
        ZlibDecoder::new(fs::File::open(entry.path())?).read_to_end(&mut blob)?;

        let contents = parse_blob(blob.as_slice()).context("failed to parse object file")?;

        let mut stdout = io::stdout().lock();
        stdout.write(contents)?;

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

    pub fn hash_object(path: impl AsRef<Path>, write: bool) -> Result<()> {
        let contents = fs::read(path.as_ref())?;
        let header = format!("blob {}\0", contents.len());

        let mut hasher = Sha1::new();

        io::copy(
            &mut header.as_bytes().chain(contents.as_slice()),
            &mut hasher,
        )?;

        let digest = hasher.finalize();
        for byte in &digest {
            print!("{byte:02x}");
        }
        println!();

        if write {
            let mut path: PathBuf = [DOT_GIT, OBJECTS, &format!("{:02x}", &digest[0])]
                .iter()
                .collect();

            if let Err(error) = fs::create_dir(&path) {
                ensure!(
                    error.kind() == io::ErrorKind::AlreadyExists,
                    "failed to create object subdirectory"
                );
            }

            path.push({
                use std::fmt::Write; //here to prevent conflict with io::Write

                let mut filename = OsString::new();
                for byte in &digest[1..] {
                    write!(&mut filename, "{byte:02x}")?;
                }
                filename
            });

            let mut file = fs::File::create(path)?;

            let mut compressor = ZlibEncoder::new(
                header.as_bytes().chain(contents.as_slice()),
                Default::default(), // default compression is level 6
            );

            io::copy(&mut compressor, &mut file)?;
        }

        Ok(())
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
