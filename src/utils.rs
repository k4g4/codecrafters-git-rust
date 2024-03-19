use std::{
    cell::Cell,
    fmt,
    fs::{self, File},
    io::{self, Read, Write},
    mem,
    path::{Path, PathBuf},
};

use anyhow::{ensure, Context};
use flate2::read::ZlibDecoder;

use crate::{parsing, CONFIG, DOT_GIT, HEAD, OBJECTS, SHA_DISPLAY_LEN, SHA_LEN};

#[derive(Clone, Copy)]
pub struct EntryDisplay {
    pub trees_only: bool,
    pub name_only: bool,
    pub abbrev: u8,
}

pub struct Entry {
    pub display: Cell<Option<EntryDisplay>>,
    pub mode: u32,
    pub hash: [u8; SHA_LEN],
    pub name: String,
    pub tree: bool,
    pub children: Option<Vec<Entry>>,
}

impl fmt::Display for Entry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let display = self
            .display
            .get()
            .expect("assigned before display is called");

        if display.trees_only && !self.tree {
            return Ok(());
        };

        if !display.name_only {
            write!(f, "{:06}\t", self.mode)?;
            write!(f, "{}\t", if self.tree { "tree" } else { "blob" })?;
            for byte in &self.hash[..display.abbrev as usize] {
                write!(f, "{byte:02x}")?;
            }
            write!(f, "\t")?;
        }

        writeln!(f, "{}", self.name)?;

        if let Some(children) = self.children.as_deref() {
            for child in children {
                child.display.set(self.display.get());
                write!(f, "{child}")?;
            }
        }

        Ok(())
    }
}

pub fn find_object(hash: &str) -> anyhow::Result<PathBuf> {
    let failed_context = || format!("failed to find {hash}");

    ensure!(hash.len() > 3, "object hash is not long enough");
    let (sha_dir, sha_file) = hash.split_at(2);

    let entries = fs::read_dir(Path::new(DOT_GIT).join(OBJECTS))?;

    let entry = entries
        .filter_map(Result::ok)
        .find(|entry| sha_dir == entry.file_name())
        .with_context(failed_context)?;

    let entries = fs::read_dir(entry.path())?;

    let entry = entries
        .filter_map(Result::ok)
        .find(|entry| {
            entry.file_name().len() == SHA_DISPLAY_LEN - 2
                && entry
                    .file_name()
                    .as_os_str()
                    .to_string_lossy()
                    .starts_with(sha_file)
        })
        .with_context(failed_context)?;

    Ok(entry.path())
}

pub fn create_object(hash: &[u8; SHA_LEN]) -> anyhow::Result<File> {
    let mut path: PathBuf = [DOT_GIT, OBJECTS, &format!("{:02x}", &hash[0])]
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

        let mut filename = String::with_capacity(SHA_DISPLAY_LEN - 2);
        for byte in &hash[1..] {
            write!(filename, "{byte:02x}")?;
        }
        filename
    });

    // remove an existing file to clear permissions
    if let Err(error) = fs::remove_file(&path) {
        ensure!(
            error.kind() == io::ErrorKind::NotFound,
            "failed to remove object"
        );
    }

    Ok(File::create(path)?)
}

pub fn tree_level(hash: &str, recurse: bool) -> anyhow::Result<Vec<Entry>> {
    let path = find_object(hash)?;

    let mut buf = vec![];
    ZlibDecoder::new(File::open(path)?).read_to_end(&mut buf)?;

    let (_, entries) = parsing::parse_tree(recurse)(&buf)?;

    Ok(entries)
}

pub fn get_head() -> anyhow::Result<Option<String>> {
    let head_file = fs::read_to_string(Path::new(DOT_GIT).join(HEAD))?;
    let head_ref_at = Path::new(DOT_GIT).join(
        head_file
            .trim()
            .strip_prefix("ref: ")
            .context("detached HEAD")?,
    );
    Ok(fs::read_to_string(head_ref_at).ok())
}

pub fn update_head(commit_hash: &str) -> anyhow::Result<()> {
    let head_file = fs::read_to_string(Path::new(DOT_GIT).join(HEAD))?;
    let head_ref_at = Path::new(DOT_GIT).join(
        head_file
            .trim()
            .strip_prefix("ref: ")
            .context("detached HEAD")?,
    );
    Ok(fs::write(head_ref_at, commit_hash)?)
}

pub fn get_config_value(section: &str, key: &str) -> anyhow::Result<Option<String>> {
    Ok(read_config()?
        .into_iter()
        .find_map(|(s, keys_values)| (s == section).then(|| keys_values))
        .and_then(|keys_values| {
            keys_values
                .into_iter()
                .find_map(|(k, value)| (k == key).then(|| value))
        }))
}

pub fn set_config_value(section: &str, key: &str, value: String) -> anyhow::Result<()> {
    let mut config = read_config()?;

    let search_result = config
        .iter_mut()
        .find_map(|(s, keys_values)| (section == s).then(|| keys_values));

    let keys_values = if let Some(keys_values) = search_result {
        keys_values
    } else {
        config.push((section.into(), vec![]));
        let (_, keys_values) = config.last_mut().expect("just pushed");
        keys_values
    };

    let search_result = keys_values
        .iter_mut()
        .find_map(|(k, value)| (key == k).then(|| value));

    if let Some(prev_value) = search_result {
        *prev_value = value;
    } else {
        keys_values.push((key.into(), value));
    }

    write_config(config)
}

pub fn list_config() -> anyhow::Result<String> {
    use std::fmt::Write; // prevent conflict with io::Write

    let mut list = String::new();
    for (section, keys_values) in read_config()? {
        let section = section.trim_end_matches('"').replace(" \"", ".");

        for (key, value) in keys_values {
            writeln!(&mut list, "{section}.{key}={value}")?;
        }
    }

    Ok(list)
}

fn read_config() -> anyhow::Result<Vec<(String, Vec<(String, String)>)>> {
    let Ok(config) = fs::read_to_string(Path::new(DOT_GIT).join(CONFIG)) else {
        return Ok(vec![]);
    };

    let mut sections = vec![];
    let mut section = vec![];
    let mut section_name = None;
    for line in config.lines() {
        let line = line.trim();

        if line.starts_with('[') && line.ends_with(']') {
            if let Some(prev_section_name) = section_name.replace(line.trim_matches(['[', ']'])) {
                sections.push((prev_section_name.into(), mem::take(&mut section)));
            }
        } else if !line.is_empty() {
            let (key, value) = line.split_once(" = ").context("invalid line in config")?;
            section.push((key.into(), value.into()));
        }
    }
    if let Some(prev_section_name) = section_name {
        sections.push((prev_section_name.into(), section));
    }

    Ok(sections)
}

fn write_config(config: Vec<(String, Vec<(String, String)>)>) -> anyhow::Result<()> {
    let mut config_file = File::create(Path::new(DOT_GIT).join(CONFIG))?;

    for (section, keys_values) in config {
        writeln!(&mut config_file, "[{section}]")?;

        for (key, value) in keys_values {
            writeln!(&mut config_file, "\t{key} = {value}")?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{env, sync::MutexGuard};

    use crate::FORCE_SINGLE_THREAD;

    use super::*;

    const CONFIG_TEST_DIR: &'static str = "config_test_dir";

    struct Setup(MutexGuard<'static, ()>);
    impl Setup {
        fn init() -> Self {
            let guard = FORCE_SINGLE_THREAD.lock().unwrap();
            let _ = fs::remove_dir_all(CONFIG_TEST_DIR);
            fs::create_dir(CONFIG_TEST_DIR).unwrap();
            env::set_current_dir(CONFIG_TEST_DIR).unwrap();
            fs::create_dir(".git").unwrap();
            fs::write(
                ".git/config",
                "\
[core]
\trepositoryformatversion = 0
\tfilemode = true
\tbare = false
\tlogallrefupdates = true
[user]
\tname = Andres Dejesus
\temail = andresdejesus123@gmail.com
",
            )
            .unwrap();
            Self(guard)
        }
    }
    impl Drop for Setup {
        fn drop(&mut self) {
            env::set_current_dir("..").unwrap();
            let _ = fs::remove_dir_all(CONFIG_TEST_DIR);
        }
    }

    #[test]
    fn read_all_config() {
        let _setup = Setup::init();

        let config = read_config().unwrap();

        assert_eq!(
            config,
            [
                (
                    "core".into(),
                    vec![
                        ("repositoryformatversion".into(), "0".into()),
                        ("filemode".into(), "true".into()),
                        ("bare".into(), "false".into()),
                        ("logallrefupdates".into(), "true".into())
                    ]
                ),
                (
                    "user".into(),
                    vec![
                        ("name".into(), "Andres Dejesus".into()),
                        ("email".into(), "andresdejesus123@gmail.com".into())
                    ]
                )
            ]
        );
    }

    #[test]
    fn read_config_values() {
        let _setup = Setup::init();

        assert_eq!(
            get_config_value("core", "filemode").unwrap().unwrap(),
            "true"
        );

        assert!(get_config_value("foo", "bar").unwrap().is_none(),);

        assert!(get_config_value("user", "age").unwrap().is_none(),);

        assert_eq!(
            get_config_value("user", "name").unwrap().unwrap(),
            "Andres Dejesus"
        );
    }

    #[test]
    fn read_and_write() {
        let _setup = Setup::init();

        let before = fs::read_to_string(".git/config").unwrap();
        let config = read_config().unwrap();

        fs::remove_file(".git/config").unwrap();
        fs::write(".git/config", "").unwrap();
        let during = fs::read_to_string(".git/config").unwrap();
        assert!(during.is_empty());

        write_config(config).unwrap();
        let after = fs::read_to_string(".git/config").unwrap();

        assert_eq!(before, after);
    }

    #[test]
    fn set_config_values() {
        let _setup = Setup::init();
        fs::write(".git/config", "").unwrap();

        set_config_value("foo", "a", "b".into()).unwrap();
        set_config_value("foo", "c", "d".into()).unwrap();
        set_config_value("bar", "a", "b".into()).unwrap();
        set_config_value("bar", "c", "d".into()).unwrap();
        set_config_value("bar", "e", "f".into()).unwrap();
        set_config_value("Baz", "1 + 1", "2".into()).unwrap();
        set_config_value("Baz", "true", "false".into()).unwrap();
        set_config_value("Baz", "up", "down".into()).unwrap();
        set_config_value("Baz", "north", "south".into()).unwrap();

        assert_eq!(
            fs::read_to_string(".git/config").unwrap(),
            "\
[foo]
\ta = b
\tc = d
[bar]
\ta = b
\tc = d
\te = f
[Baz]
\t1 + 1 = 2
\ttrue = false
\tup = down
\tnorth = south
"
        );
    }
}
