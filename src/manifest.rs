// use std::collections::HashSet;
use async_std::fs::*;
use async_std::path::{PathBuf, Path};
use crate::prelude::*;
use regex::Regex;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Manifest {
    pub site : Site,
}

impl Manifest {
    pub async fn locate(location: Option<String>) -> Result<PathBuf> {
        let cwd = current_dir().await;

        let location = if let Some(location) = location {
            if location.starts_with("~/") {
                home::home_dir().expect("unable to get home directory").join(&location[2..]).into()
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
            &location.join("wahoo.toml")
        ];

        for location in locations.iter() {
            match location.canonicalize().await {
                Ok(location) => {
                    if location.is_file().await {
                        return Ok(location)
                    }
                }, 
                _ => { }
            }
        }

        Err(format!("Unable to locate 'wahoo.toml' manifest").into())
    }
    
    pub async fn load(toml : &PathBuf) -> Result<Manifest> {
        let nw_toml = read_to_string(toml).await?;
        let manifest: Manifest = match toml::from_str(&nw_toml) {
            Ok(manifest) => manifest,
            Err(err) => {
                return Err(format!("Error loading nw.toml: {}", err).into());
            }
        };    

        manifest.sanity_checks()?;

        Ok(manifest)
    }
    
    pub fn sanity_checks(&self) -> Result<()> {

        let regex = Regex::new(r"^[^\s]*[a-z0-9-_]*$").unwrap();
        if !regex.is_match(&self.site.name) {
            return Err(format!("invalid application name '{}'", self.site.name).into());
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Site {
    pub name : String,
    pub title : String,
    pub target_folder : Option<String>,
}
