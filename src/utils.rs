use crate::prelude::*;
use regex::Regex;

pub async fn search_upwards(folder: &Path, filename: &str) -> Option<PathBuf> {
    let mut folder = folder.to_path_buf();

    loop {
        let file_path = folder.join(filename);
        if file_path.is_file() {
            return Some(file_path);
        }

        if let Some(parent) = folder.parent() {
            folder = parent.to_path_buf();
        } else {
            return None;
        }
    }
}

pub async fn current_dir() -> PathBuf {
    std::env::current_dir().unwrap()
}

// pub async fn find_file(folder: &Path,files: &[&str]) -> Result<PathBuf> {
pub async fn find_file(folder: &Path, files: &[String]) -> Result<PathBuf> {
    for file in files {
        let path = folder.join(file);
        if let Ok(path) = path.canonicalize() {
            if path.is_file() {
                return Ok(path);
            }
        }
    }
    Err(format!(
        "Unable to locate any of the files: {} \nfrom {:?} directory",
        files.join(", "),
        folder.to_str().unwrap_or("")
    )
    .into())
}

pub fn get_env_defs(strings: &Vec<String>) -> Result<Vec<(String, String)>> {
    let regex = Regex::new(r"([^=]+?)=(.+)").unwrap();

    let mut parsed_strings = Vec::new();

    for string in strings {
        let captures = regex.captures(string).unwrap();
        if captures.len() != 2 {
            return Err(format!("Error parsing the environment string: '{string}'").into());
        }
        let a = captures[1].to_string();
        let b = captures[2].to_string();

        parsed_strings.push((a, b));
    }

    Ok(parsed_strings)
}

pub fn is_hidden<P>(path: P) -> bool
where
    P: AsRef<Path>,
{
    path.as_ref()
        .components()
        .any(|f| f.as_os_str().to_string_lossy().starts_with('.'))
}
