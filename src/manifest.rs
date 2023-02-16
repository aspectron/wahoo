use crate::prelude::*;
use async_std::fs::*;
// use async_std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Manifest {
    pub toml: toml::Value,
    pub settings: Option<Settings>,
    pub sections: Option<HashMap<String, Section>>,
    pub imports: Vec<PathBuf>,
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

    pub async fn load_toml<F, P>(folder: F, toml_file: P) -> Result<toml::Value>
    where
        F: AsRef<Path>,
        P: AsRef<Path>,
    {
        let toml_file = folder.as_ref().join(toml_file);
        // log_info!("TOML","loading {}", toml_file.display());
        let toml_file = toml_file.canonicalize().map_err(|err| -> Error {
            format!("Unable to load `{}`: {err}", toml_file.display()).into()
        })?;
        let toml_text = read_to_string(&toml_file).await?;
        let toml: toml::Value = match toml::from_str(&toml_text) {
            Ok(manifest) => manifest,
            Err(err) => {
                return Err(format!("Error loading `{}`: {err}", toml_file.display()).into());
            }
        };
        Ok(toml)
    }

    pub async fn load(toml_file: &PathBuf) -> Result<Manifest> {
        let folder = toml_file.parent().unwrap();
        let toml_text = read_to_string(toml_file).await?;
        let mut toml: toml::Value = match toml::from_str(&toml_text) {
            Ok(manifest) => manifest,
            Err(err) => {
                return Err(format!("Error loading wahoo.toml: {err}").into());
            }
        };
        // println!("loading settings...");
        let settings = if let Some(settings) = toml.get("settings") {
            let settings: Settings = settings.clone().try_into()?;
            Some(settings)
        } else {
            None
        };
        // println!("loading sections...");

        let mut imports = vec![];
        if let Some(settings) = &settings {
            if let Some(import_list) = &settings.import {
                for import in import_list.iter() {
                    let toml_import = Self::load_toml(folder, import).await?;
                    let import_path = folder.join(import).canonicalize().unwrap();
                    imports.push(import_path);
                    // println!("{:#?}", toml);
                    let target = toml.as_table_mut().unwrap();
                    for (k, v) in toml_import.as_table().unwrap().iter() {
                        target.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        // panic!();
        let sections = if let Some(Settings { sections : Some(sections_list), .. }) = &settings {//toml.get("section") {
            // println!("loading section refs: {:#?}",sections);
            // let section_refs: HashMap<String, SectionReference> = sections.clone().try_into()?;

            let mut sections: HashMap<String, Section> = HashMap::new();

            // for (name, section_ref) in section_refs.into_iter() {
            for section in sections_list.into_iter() {
                log_info!("Section", "loading {section}");
                // if let Some(index) = &section_ref.index {
                let section_toml = Self::load_toml(folder, &section).await?;
                let section_path = folder.join(&section).canonicalize().unwrap();
                imports.push(section_path);
                let settings = if let Some(settings) = section_toml.get("settings") {
                    let settings: SectionSettings = settings.clone().try_into()?;
                    Some(settings)
                } else {
                    None
                };

                // let name = if le
                let name = Path::new(section).file_stem().unwrap().to_str().unwrap().to_string();

                sections.insert(
                    name.clone(),
                    Section {
                        name,
                        settings,
                        toml: section_toml,
                    },
                );
                
            }
            Some(sections)
        } else {
            None
        };

        //let table = toml.as_table().unwrap();
        //println!("{:#?}", table);
        //println!("settings: {:#?}", settings);
        // println!("manifest loaded...");
        Ok(Manifest {
            toml,
            settings,
            sections,
            imports,
        })
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub ignore: Option<Vec<String>>,
    pub languages: Option<Vec<String>>,
    pub map: Option<Vec<DataMap>>,
    pub error_404: Option<String>,
    pub error_500: Option<String>,
    pub import: Option<Vec<String>>,
    pub watch: Option<Vec<String>>,
    pub sections: Option<Vec<String>>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct DataMap {
    pub data: String,
    pub templates: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct  Section {
    pub name: String,
    // pub index: Option<String>,
    pub settings: Option<SectionSettings>,
    pub toml: toml::Value,
    // pub templates: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SectionSettings {
    /// Section title
    pub title: Option<String>,
    /// Section contents folder
    pub folder: Option<String>,
    /// Section description
    // description: Option<String>,
    pub index: Option<String>,
    pub template: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SectionReference {
    // pub name: String,
    pub index: Option<String>,
    pub templates: Option<String>,
}

// impl From<(&str, &toml::Value)> for Section {
//     fn from((name, value): (&str, &toml::Value)) -> Self {
//         let index = value.get("index").map(|v| v.as_str().map(|s|s.to_string()).expect("section index must be a string"));
//         // let templates = value.get("templates").map(|v| v.as_str().map(|s|s.to_string()).expect("section templates must be a string"));

//         Section {
//             name: name.to_string(),
//             index,
//             // templates,
//         }
//     }
// }
