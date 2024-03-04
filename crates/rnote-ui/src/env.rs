// Imports
use crate::config;
use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};
use winreg;

pub(crate) fn lib_dir() -> anyhow::Result<PathBuf> {
    if cfg!(target_os = "windows") {
        let exec_dir = exec_parent_dir()?;

        Ok(exec_dir.join("..\\lib"))
    } else if cfg!(target_os = "macos") {
        let canonicalized_exec_dir = exec_parent_dir()?.canonicalize()?;

        if macos_is_in_app_bundle(&canonicalized_exec_dir) {
            Ok(canonicalized_exec_dir.join("../Resources/lib"))
        } else {
            Ok(PathBuf::from(config::LIBDIR))
        }
    } else {
        Ok(PathBuf::from(config::LIBDIR))
    }
}

pub(crate) fn data_dir() -> anyhow::Result<PathBuf> {
    if cfg!(target_os = "windows") {
        let exec_dir = exec_parent_dir()?;

        Ok(exec_dir.join("..\\share"))
    } else if cfg!(target_os = "macos") {
        let canonicalized_exec_dir = exec_parent_dir()?.canonicalize()?;

        if macos_is_in_app_bundle(&canonicalized_exec_dir) {
            Ok(canonicalized_exec_dir.join("../Resources/share"))
        } else {
            Ok(PathBuf::from(config::DATADIR))
        }
    } else {
        Ok(PathBuf::from(config::DATADIR))
    }
}

pub(crate) fn pkg_data_dir() -> anyhow::Result<PathBuf> {
    Ok(data_dir()?.join(config::APP_NAME))
}

pub(crate) fn locale_dir() -> anyhow::Result<PathBuf> {
    if cfg!(target_os = "windows") {
        let exec_dir = exec_parent_dir()?;

        Ok(exec_dir.join("..\\share\\locale"))
    } else if cfg!(target_os = "macos") {
        let canonicalized_exec_dir = exec_parent_dir()?.canonicalize()?;

        if macos_is_in_app_bundle(&canonicalized_exec_dir) {
            Ok(canonicalized_exec_dir.join("../Resources/share/locale"))
        } else {
            Ok(PathBuf::from(config::LOCALEDIR))
        }
    } else {
        Ok(PathBuf::from(config::LOCALEDIR))
    }
}

/// depending on the target platform we need to set some env vars on startup
pub(crate) fn setup_env() -> anyhow::Result<()> {
    if cfg!(target_os = "windows") {
        let data_dir = data_dir()?;
        let lib_dir = lib_dir()?;

        std::env::set_var("XDG_DATA_DIRS", data_dir.clone());

        #[cfg(target_os = "windows")]
        {
            let key = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
            let app_seting_key = key.open_subkey("software\\GSettings\\com\\github\\flxzt\\rnote");

            let gdk_workaround = match app_seting_key {
                Err(_) => false,
                Ok(key) => key.get_value("gdk-scale").unwrap_or(0u32) == 1,
            };

            tracing::debug!("{:?}", gdk_workaround);
            if gdk_workaround {
                std::env::set_var("GDK_SCALE", "2");
            }
        }

        std::env::set_var(
            "GDK_PIXBUF_MODULEDIR",
            lib_dir.join("gdk-pixbuf-2.0\\2.10.0\\loaders"),
        );
    } else if cfg!(target_os = "macos") {
        let canonicalized_exec_dir = exec_parent_dir()?.canonicalize()?;

        if macos_is_in_app_bundle(canonicalized_exec_dir) {
            let data_dir = data_dir()?;
            let lib_dir = lib_dir()?;

            std::env::set_var("XDG_DATA_DIRS", data_dir);
            std::env::set_var(
                "GDK_PIXBUF_MODULE_FILE",
                lib_dir.join("gdk-pixbuf-2.0/2.10.0/loaders.cache"),
            );
        }
    }
    Ok(())
}

fn exec_parent_dir() -> anyhow::Result<PathBuf> {
    Ok(std::env::current_exe()?
        .parent()
        .ok_or(anyhow::anyhow!(
            "could not get parent dir of executable path"
        ))?
        .to_path_buf())
}

// this returns true when the app is packaged as a relocatable application bundle
fn macos_is_in_app_bundle(canonicalized_exec_dir: impl AsRef<Path>) -> bool {
    canonicalized_exec_dir
        .as_ref()
        .components()
        .zip(canonicalized_exec_dir.as_ref().components().skip(1))
        .any(|(a, b)| {
            if let (Component::Normal(a), Component::Normal(b)) = (a, b) {
                a == OsStr::new("Contents") && b == OsStr::new("MacOS")
            } else {
                false
            }
        })
}
