//! Application-owned static mounts using Claviron's file service.
use std::path::PathBuf;
use std::sync::Arc;

use claviron::axum::Router;
use claviron::axum::body::Body;
use claviron::axum::extract::State;
use claviron::axum::middleware::{Next, from_fn_with_state};
use claviron::files::FileSite;
use claviron::http::config::statics::StaticConfig;
use http::{Method, StatusCode};
use serde::Deserialize;

use crate::routing::{AppRequest, AppResponse, AppService};

/// Explicit asset mounts for one application; paths are working-directory relative.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Files {
    pub root: PathBuf,
    pub statics: Vec<StaticConfig>,
}
impl Files {
    pub(crate) fn validate(&self) -> crate::Result<()> {
        if self.root.as_os_str().is_empty() {
            return Err("files.root must not be empty".into());
        }
        for mount in &self.statics {
            if !mount.path.starts_with('/') || mount.path.contains(['?', '#']) {
                return Err("files.statics paths must be absolute URL paths".into());
            }
            if mount.dir.is_empty() {
                return Err("files.statics directories must not be empty".into());
            }
        }
        Ok(())
    }
}
impl FileSite for Files {
    fn root(&self) -> PathBuf {
        self.root.clone()
    }

    fn indexes(&self) -> &[String] {
        &[]
    }

    fn try_files(&self) -> bool {
        false
    }

    fn spa(&self) -> Option<&str> {
        None
    }

    fn statics(&self) -> Option<&[StaticConfig]> {
        Some(&self.statics)
    }

    fn has_static_index(&self) -> bool {
        false
    }
}

pub(crate) fn wrap(
    files: Option<Arc<Files>>,
    service: AppService,
) -> AppService {
    let Some(files) = files else { return service };
    let site: Arc<dyn FileSite> = files;
    AppService::new(
        Router::new()
            .fallback_service(service)
            .layer(from_fn_with_state(site, serve)),
    )
}

async fn serve(
    site: State<Arc<dyn FileSite>>,
    request: AppRequest,
    next: Next,
) -> Result<AppResponse, StatusCode> {
    // The shared file primitive has no method argument. Mutation requests
    // must keep reaching the application even if their path names an asset.
    let head = request.method() == Method::HEAD;
    if !head && request.method() != Method::GET {
        return Ok(next.run(request).await);
    }
    let mut response = claviron::files::axum_static_mount_handler(site, request, next).await?;
    if head {
        *response.body_mut() = Body::empty();
    }
    Ok(response)
}

#[cfg(test)]
#[path = "../tests/files.rs"]
mod tests;
