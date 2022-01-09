// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use crate::{api::path::BaseDirectory, Runtime};
#[cfg(path_all)]
use crate::{Env, Manager};
use std::path::PathBuf;
#[cfg(path_all)]
use std::path::{Component, Path, MAIN_SEPARATOR};

use super::InvokeContext;
use serde::Deserialize;
use tauri_macros::{module_command_handler, CommandModule};

/// The API descriptor.
#[derive(Deserialize, CommandModule)]
#[serde(tag = "cmd", rename_all = "camelCase")]
pub enum Cmd {
  ResolvePath {
    path: String,
    directory: Option<BaseDirectory>,
  },
  Resolve {
    paths: Vec<String>,
  },
  Normalize {
    path: String,
  },
  Join {
    paths: Vec<String>,
  },
  Dirname {
    path: String,
  },
  Extname {
    path: String,
  },
  Basename {
    path: String,
    ext: Option<String>,
  },
  IsAbsolute {
    path: String,
  },
}

impl Cmd {
  #[module_command_handler(path_all, "path > all")]
  fn resolve_path<R: Runtime>(
    context: InvokeContext<R>,
    path: String,
    directory: Option<BaseDirectory>,
  ) -> crate::Result<PathBuf> {
    crate::api::path::resolve_path(
      &context.config,
      &context.package_info,
      context.window.state::<Env>().inner(),
      path,
      directory,
    )
    .map_err(Into::into)
  }

  #[module_command_handler(path_all, "path > all")]
  fn resolve<R: Runtime>(_context: InvokeContext<R>, paths: Vec<String>) -> crate::Result<PathBuf> {
    // Start with current directory then start adding paths from the vector one by one using `PathBuf.push()` which
    // will ensure that if an absolute path is encountered in the iteration, it will be used as the current full path.
    //
    // examples:
    // 1. `vec!["."]` or `vec![]` will be equal to `std::env::current_dir()`
    // 2. `vec!["/foo/bar", "/tmp/file", "baz"]` will be equal to `PathBuf::from("/tmp/file/baz")`
    let mut path = std::env::current_dir()?;
    for p in paths {
      path.push(p);
    }
    Ok(normalize_path(&path))
  }

  #[module_command_handler(path_all, "path > all")]
  fn normalize<R: Runtime>(_context: InvokeContext<R>, path: String) -> crate::Result<String> {
    let mut p = normalize_path_no_absolute(Path::new(&path))
      .to_string_lossy()
      .to_string();
    Ok(
      // Node.js behavior is to return `".."` for `normalize("..")`
      // and `"."` for `normalize("")` or `normalize(".")`
      if p.is_empty() && path == ".." {
        "..".into()
      } else if p.is_empty() && path == "." {
        ".".into()
      } else {
        // Add a trailing separator if the path passed to this functions had a trailing separator. That's how Node.js behaves.
        if (path.ends_with('/') || path.ends_with('\\'))
          && (!p.ends_with('/') || !p.ends_with('\\'))
        {
          p.push(MAIN_SEPARATOR);
        }
        p
      },
    )
  }

  #[module_command_handler(path_all, "path > all")]
  fn join<R: Runtime>(_context: InvokeContext<R>, mut paths: Vec<String>) -> crate::Result<String> {
    let path = PathBuf::from(
      paths
        .iter_mut()
        .map(|p| {
          // Add a `MAIN_SEPARATOR` if it doesn't already have one.
          // Doing this to ensure that the vector elements are separated in
          // the resulting string so path.components() can work correctly when called
          // in `normalize_path_no_absolute()` later on.
          if !p.ends_with('/') && !p.ends_with('\\') {
            p.push(MAIN_SEPARATOR);
          }
          p.to_string()
        })
        .collect::<String>(),
    );

    let p = normalize_path_no_absolute(&path)
      .to_string_lossy()
      .to_string();
    Ok(if p.is_empty() { ".".into() } else { p })
  }

  #[module_command_handler(path_all, "path > all")]
  fn dirname<R: Runtime>(_context: InvokeContext<R>, path: String) -> crate::Result<PathBuf> {
    match Path::new(&path).parent() {
      Some(p) => Ok(p.to_path_buf()),
      None => Err(crate::Error::FailedToExecuteApi(crate::api::Error::Path(
        "Couldn't get the parent directory".into(),
      ))),
    }
  }

  #[module_command_handler(path_all, "path > all")]
  fn extname<R: Runtime>(_context: InvokeContext<R>, path: String) -> crate::Result<String> {
    match Path::new(&path)
      .extension()
      .and_then(std::ffi::OsStr::to_str)
    {
      Some(p) => Ok(p.to_string()),
      None => Err(crate::Error::FailedToExecuteApi(crate::api::Error::Path(
        "Couldn't get the extension of the file".into(),
      ))),
    }
  }

  #[module_command_handler(path_all, "path > all")]
  fn basename<R: Runtime>(
    _context: InvokeContext<R>,
    path: String,
    ext: Option<String>,
  ) -> crate::Result<String> {
    match Path::new(&path)
      .file_name()
      .and_then(std::ffi::OsStr::to_str)
    {
      Some(p) => Ok(if let Some(ext) = ext {
        p.replace(ext.as_str(), "")
      } else {
        p.to_string()
      }),
      None => Err(crate::Error::FailedToExecuteApi(crate::api::Error::Path(
        "Couldn't get the basename".into(),
      ))),
    }
  }

  #[module_command_handler(path_all, "path > all")]
  fn is_absolute<R: Runtime>(_context: InvokeContext<R>, path: String) -> crate::Result<bool> {
    Ok(Path::new(&path).is_absolute())
  }
}

/// Normalize a path, removing things like `.` and `..`, this snippet is taken from cargo's paths util.
/// https://github.com/rust-lang/cargo/blob/46fa867ff7043e3a0545bf3def7be904e1497afd/crates/cargo-util/src/paths.rs#L73-L106
#[cfg(path_all)]
fn normalize_path(path: &Path) -> PathBuf {
  let mut components = path.components().peekable();
  let mut ret = if let Some(c @ Component::Prefix(..)) = components.peek().cloned() {
    components.next();
    PathBuf::from(c.as_os_str())
  } else {
    PathBuf::new()
  };

  for component in components {
    match component {
      Component::Prefix(..) => unreachable!(),
      Component::RootDir => {
        ret.push(component.as_os_str());
      }
      Component::CurDir => {}
      Component::ParentDir => {
        ret.pop();
      }
      Component::Normal(c) => {
        ret.push(c);
      }
    }
  }
  ret
}

/// Normalize a path, removing things like `.` and `..`, this snippet is taken from cargo's paths util but
/// slightly modified to not resolve absolute paths.
/// https://github.com/rust-lang/cargo/blob/46fa867ff7043e3a0545bf3def7be904e1497afd/crates/cargo-util/src/paths.rs#L73-L106
#[cfg(path_all)]
fn normalize_path_no_absolute(path: &Path) -> PathBuf {
  let mut components = path.components().peekable();
  let mut ret = if let Some(c @ Component::Prefix(..)) = components.peek().cloned() {
    components.next();
    PathBuf::from(c.as_os_str())
  } else {
    PathBuf::new()
  };

  for component in components {
    match component {
      Component::Prefix(..) => unreachable!(),
      Component::RootDir => {
        ret.push(component.as_os_str());
      }
      Component::CurDir => {}
      Component::ParentDir => {
        ret.pop();
      }
      Component::Normal(c) => {
        // Using PathBuf::push here will replace the whole path if an absolute path is encountered
        // which is not the intended behavior, so instead of that, convert the current resolved path
        // to a string and do simple string concatenation with the current component then convert it
        // back to a PathBuf
        let mut p = ret.to_string_lossy().to_string();
        // Only add a separator if it doesn't have one already or if current normalized path is empty,
        // this ensures it won't have an unwanted leading separator
        if !p.is_empty() && !p.ends_with('/') && !p.ends_with('\\') {
          p.push(MAIN_SEPARATOR);
        }
        if let Some(c) = c.to_str() {
          p.push_str(c);
        }
        ret = PathBuf::from(p);
      }
    }
  }
  ret
}
