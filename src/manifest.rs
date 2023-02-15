use crate::prelude::*;
use async_std::fs::*;
// use async_std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Manifest {
    pub toml: toml::Value,
    pub settings: Option<Settings>,
}

impl Manifest {
    pub async fn locate(location: Option<String>) -> Result<PathBuf> {
        let cwd = current_dir().await;

        let location = if let Some(location) = location {
            if let Some(stripped) = location.strip_prefix("~/") {
                home::home_dir()
                    .expect("unable to get home directory")
                    .join(stripped)
            } else {
                let location = Path::new(&location).to_path_buf();
                if location.is_absolute() {
                    location
                } else {
                    cwd.join(&location)
                }
            }
        } else {
            cwd
        };

        let locations = [
            &location,
            &location.with_extension("toml"),
            &location.join("wahoo.toml"),
        ];

        for location in locations.iter() {
            if let Ok(location) = location.canonicalize() {
                if location.is_file() {
                    return Ok(location);
                }
            }
        }

        Err("Unable to locate 'wahoo.toml' manifest".into())
    }

    pub async fn load(toml_file: &PathBuf) -> Result<Manifest> {
        let toml_text = read_to_string(toml_file).await?;
        let toml: toml::Value = match toml::from_str(&toml_text) {
            Ok(manifest) => manifest,
            Err(err) => {
                return Err(format!("Error loading wahoo.toml: {err}").into());
            }
        };

        let settings = if let Some(settings) = toml.get("settings") {
            let settings: Settings = settings.clone().try_into()?;
            Some(settings)
        } else {
            None
        };

        //let table = toml.as_table().unwrap();
        //println!("{:#?}", table);
        //println!("settings: {:#?}", settings);

        Ok(Manifest { toml, settings })
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub ignore: Option<Vec<String>>,
    pub languages: Option<Vec<String>>,
    pub map: Option<Vec<DataMap>>,
    pub error_404: Option<String>,
    pub error_500: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct DataMap {
    pub data: String,
    pub templates: String,
}
