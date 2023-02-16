use crate::prelude::*;

#[derive(Default, Debug)]
pub struct Options {
    pub server: bool,
    pub verbose: bool,
}

#[derive(Debug)]
pub struct Context {
    pub manifest: Manifest,
    pub manifest_toml: PathBuf,
    pub site_folder: PathBuf,
    pub src_folder: PathBuf,
    pub project_folder: PathBuf,
    pub options: Options,
}

impl Context {
    pub async fn create(location: Option<String>, options: Options) -> Result<Context> {
        let manifest_toml = Manifest::locate(location).await?;

        let manifest = Manifest::load(&manifest_toml).await?;
        let project_folder = manifest_toml.parent().unwrap().to_path_buf();

        let site_folder = project_folder.join("site");
        let src_folder = project_folder.join("src");

        if options.verbose {
            log_info!("Manifest", "`{}`", manifest_toml.to_str().unwrap());
            log_info!("Project", "`{}`", src_folder.to_str().unwrap());
            log_info!("Target", "`{}`", site_folder.to_str().unwrap());
        }

        let ctx = Context {
            manifest,
            manifest_toml,
            project_folder,
            site_folder,
            src_folder,
            options,
        };

        Ok(ctx)
    }

    pub async fn ensure_folders(&self) -> Result<()> {
        let folders = [&self.site_folder];
        for folder in folders {
            if !std::path::Path::new(folder).exists() {
                std::fs::create_dir_all(folder)?;
            }
        }

        Ok(())
    }

    pub async fn clean(&self) -> Result<()> {
        if self.site_folder.exists() {
            // log_info!("Cleaning","`{}`",self.target_folder.display());

            for entry in std::fs::read_dir(&self.site_folder)? {
                let path = entry?.path();
                if path.is_dir() {
                    if !path
                        .file_stem()
                        .map(|stem| stem.to_str().unwrap() == "node_modules")
                        .unwrap_or(false)
                    {
                        async_std::fs::remove_dir_all(&path).await?;
                    }
                } else {
                    std::fs::remove_file(path)?;
                }
            }

            // async_std::fs::remove_dir_all(&self.target_folder).await?;
        }
        Ok(())
    }

    pub fn settings(&self) -> Settings {
        let default_settings = Settings::default();
        let settings = self.manifest.settings.as_ref().unwrap_or(&default_settings);

        settings.clone()
    }

    pub fn sections(&self) -> Option<HashMap<String, toml::Value>> {
        if let Some(sections) = self.manifest.sections.as_ref() {
            let mut result = HashMap::new();
            for (key, section) in sections {
                result.insert(key.clone(), section.toml.clone());
            }
            Some(result)
        } else {
            None
        }
    }
}
