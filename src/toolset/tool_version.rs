use std::fmt::{Display, Formatter};
use std::fs;
use std::sync::Arc;

use color_eyre::eyre::Result;

use crate::config::{Config, Settings};
use crate::dirs;
use crate::plugins::{InstallType, Plugin};
use crate::runtimes::RuntimeVersion;
use crate::ui::progress_report::ProgressReport;

/// represents a single version of a tool for a particular plugin
#[derive(Debug, Clone)]
pub struct ToolVersion {
    pub plugin_name: String,
    pub r#type: ToolVersionType,
    pub rtv: Option<RuntimeVersion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ToolVersionType {
    Version(String),
    Prefix(String),
    Ref(String),
    Path(String),
    System,
}

impl ToolVersion {
    pub fn new(plugin_name: String, r#type: ToolVersionType) -> Self {
        Self {
            plugin_name,
            r#type,
            rtv: None,
        }
    }

    pub fn resolve(&mut self, settings: &Settings, plugin: Arc<Plugin>) -> Result<()> {
        if self.rtv.is_none() {
            self.rtv = match &self.r#type {
                ToolVersionType::Version(v) => self.resolve_version(settings, plugin, v)?,
                ToolVersionType::Prefix(v) => self.resolve_prefix(settings, plugin, v)?,
                ToolVersionType::Ref(r) => self.resolve_ref(plugin, r)?,
                ToolVersionType::Path(path) => self.resolve_path(plugin, path)?,
                ToolVersionType::System => None,
            };
        }
        Ok(())
    }

    fn resolve_version(
        &self,
        settings: &Settings,
        plugin: Arc<Plugin>,
        v: &str,
    ) -> Result<Option<RuntimeVersion>> {
        let v = resolve_alias(settings, plugin.clone(), v)?;
        match v.split_once(':') {
            Some(("ref", r)) => {
                return self.resolve_ref(plugin, r);
            }
            Some(("path", p)) => {
                return self.resolve_path(plugin, p);
            }
            Some(("prefix", p)) => {
                return self.resolve_prefix(settings, plugin, p);
            }
            _ => (),
        }

        if dirs::INSTALLS.join(&plugin.name).join(&v).exists() {
            // if the version is already installed, no need to fetch all of the remote versions
            let rtv = RuntimeVersion::new(plugin, InstallType::Version(v));
            return Ok(Some(rtv));
        }

        let matches = plugin.list_versions_matching(settings, &v)?;
        if matches.contains(&v) {
            let rtv = RuntimeVersion::new(plugin, InstallType::Version(v));
            Ok(Some(rtv))
        } else {
            self.resolve_prefix(settings, plugin, &v)
        }
    }

    fn resolve_prefix(
        &self,
        settings: &Settings,
        plugin: Arc<Plugin>,
        prefix: &str,
    ) -> Result<Option<RuntimeVersion>> {
        let matches = plugin.list_versions_matching(settings, prefix)?;
        let v = match matches.last() {
            Some(v) => v,
            None => prefix,
            // None => Err(VersionNotFound(plugin.name.clone(), prefix.to_string()))?,
        };
        let rtv = RuntimeVersion::new(plugin, InstallType::Version(v.to_string()));
        Ok(Some(rtv))
    }

    fn resolve_ref(&self, plugin: Arc<Plugin>, r: &str) -> Result<Option<RuntimeVersion>> {
        let rtv = RuntimeVersion::new(plugin, InstallType::Ref(r.to_string()));
        Ok(Some(rtv))
    }

    fn resolve_path(&self, plugin: Arc<Plugin>, path: &str) -> Result<Option<RuntimeVersion>> {
        let path = fs::canonicalize(path)?;
        let rtv = RuntimeVersion::new(plugin, InstallType::Path(path));
        Ok(Some(rtv))
    }

    pub fn is_missing(&self) -> bool {
        match self.rtv {
            Some(ref rtv) => !rtv.is_installed(),
            None => true,
        }
    }

    pub fn install(&mut self, config: &Config, pr: ProgressReport) -> Result<()> {
        match self.r#type {
            ToolVersionType::Version(_) | ToolVersionType::Prefix(_) | ToolVersionType::Ref(_) => {
                self.rtv.as_ref().unwrap().install(config, pr)
            }
            _ => Ok(()),
        }
    }
}

impl Display for ToolVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}@{}", &self.plugin_name, &self.r#type)
    }
}

impl Display for ToolVersionType {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            ToolVersionType::Version(v) => write!(f, "{v}"),
            ToolVersionType::Prefix(p) => write!(f, "prefix:{p}"),
            ToolVersionType::Ref(r) => write!(f, "ref:{r}"),
            ToolVersionType::Path(p) => write!(f, "path:{p}"),
            ToolVersionType::System => write!(f, "system"),
        }
    }
}

pub fn resolve_alias(settings: &Settings, plugin: Arc<Plugin>, v: &str) -> Result<String> {
    if let Some(plugin_aliases) = settings.aliases.get(&plugin.name) {
        if let Some(alias) = plugin_aliases.get(v) {
            return Ok(alias.clone());
        }
    }
    if let Some(alias) = plugin.get_aliases(settings)?.get(v) {
        return Ok(alias.clone());
    }
    Ok(v.to_string())
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_str_eq;

    use super::*;

    #[test]
    fn test_tool_version_display() {
        let foo = "foo".to_string();
        let tv = ToolVersion::new(foo.clone(), ToolVersionType::Version("1.2.3".to_string()));
        assert_str_eq!(tv.to_string(), "foo@1.2.3");
        let tv = ToolVersion::new(foo.clone(), ToolVersionType::Prefix("1.2.3".to_string()));
        assert_str_eq!(tv.to_string(), "foo@prefix:1.2.3");
        let tv = ToolVersion::new(foo.clone(), ToolVersionType::Ref("master".to_string()));
        assert_str_eq!(tv.to_string(), "foo@ref:master");
        let tv = ToolVersion::new(foo.clone(), ToolVersionType::Path("~".to_string()));
        assert_str_eq!(tv.to_string(), "foo@path:~");
        let tv = ToolVersion::new(foo, ToolVersionType::System);
        assert_str_eq!(tv.to_string(), "foo@system");
    }
}
