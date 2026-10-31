//! Transporte local: socket Unix en Linux, named pipe en Windows.
//! Solo el usuario dueño puede conectarse.

use std::io;
use tokio::io::{AsyncRead, AsyncWrite};

/// Conexión ya establecida, sin importar el sistema.
pub trait AsyncStream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> AsyncStream for T {}
pub type Stream = Box<dyn AsyncStream>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Endpoint {
    #[cfg(unix)]
    Unix(std::path::PathBuf),
    #[cfg(windows)]
    Pipe(String),
}

impl Endpoint {
    /// Dónde escucha el daemon del usuario actual.
    pub fn default_for_user() -> io::Result<Self> {
        let app = &miraishin_brand::get().app_id;
        #[cfg(unix)]
        {
            // XDG_RUNTIME_DIR (dentro de Flatpak, el de la sandbox). Si no hay, la caché del usuario.
            let base = dirs::runtime_dir()
                .or_else(dirs::cache_dir)
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::NotFound, "sin carpeta de ejecución")
                })?;
            Ok(Self::Unix(base.join(app).join("daemon.sock")))
        }
        #[cfg(windows)]
        {
            let user: String = std::env::var("USERNAME")
                .unwrap_or_default()
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
                .collect();
            Ok(Self::Pipe(format!(r"\\.\pipe\{app}-{user}")))
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BindError {
    #[error("ya hay un daemon escuchando")]
    AlreadyRunning,
    #[error(transparent)]
    Io(#[from] io::Error),
}

pub struct Listener {
    #[cfg(unix)]
    inner: tokio::net::UnixListener,
    #[cfg(unix)]
    path: std::path::PathBuf,
    #[cfg(windows)]
    name: String,
    #[cfg(windows)]
    next: tokio::net::windows::named_pipe::NamedPipeServer,
}

impl Listener {
    /// Empieza a escuchar. Si otro daemon ya responde, `AlreadyRunning`; si quedó un socket
    /// huérfano de un daemon caído, se reemplaza.
    pub async fn bind(endpoint: &Endpoint) -> Result<Self, BindError> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let Endpoint::Unix(path) = endpoint;
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
                std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
            }
            if path.exists() {
                if tokio::net::UnixStream::connect(path).await.is_ok() {
                    return Err(BindError::AlreadyRunning);
                }
                std::fs::remove_file(path)?;
            }
            let inner = tokio::net::UnixListener::bind(path)?;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
            Ok(Self {
                inner,
                path: path.clone(),
            })
        }
        #[cfg(windows)]
        {
            use tokio::net::windows::named_pipe::ServerOptions;
            let Endpoint::Pipe(name) = endpoint;
            // `first_pipe_instance` falla si otro proceso ya creó el pipe: hay un daemon.
            // La ACL por defecto del pipe solo deja entrar al mismo usuario (y a administradores).
            let next = ServerOptions::new()
                .first_pipe_instance(true)
                .reject_remote_clients(true)
                .create(name)
                .map_err(|e| match e.kind() {
                    io::ErrorKind::PermissionDenied | io::ErrorKind::AlreadyExists => {
                        BindError::AlreadyRunning
                    }
                    _ => BindError::Io(e),
                })?;
            Ok(Self {
                name: name.clone(),
                next,
            })
        }
    }

    pub async fn accept(&mut self) -> io::Result<Stream> {
        #[cfg(unix)]
        {
            let (s, _) = self.inner.accept().await?;
            Ok(Box::new(s))
        }
        #[cfg(windows)]
        {
            use tokio::net::windows::named_pipe::ServerOptions;
            self.next.connect().await?;
            let fresh = ServerOptions::new()
                .reject_remote_clients(true)
                .create(&self.name)?;
            let connected = std::mem::replace(&mut self.next, fresh);
            Ok(Box::new(connected))
        }
    }
}

#[cfg(unix)]
impl Drop for Listener {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

pub async fn connect(endpoint: &Endpoint) -> io::Result<Stream> {
    #[cfg(unix)]
    {
        let Endpoint::Unix(path) = endpoint;
        Ok(Box::new(tokio::net::UnixStream::connect(path).await?))
    }
    #[cfg(windows)]
    {
        use tokio::net::windows::named_pipe::ClientOptions;
        const ERROR_PIPE_BUSY: i32 = 231;
        let Endpoint::Pipe(name) = endpoint;
        let mut tries = 0;
        loop {
            match ClientOptions::new().open(name) {
                Ok(c) => return Ok(Box::new(c)),
                Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY) && tries < 50 => {
                    tries += 1;
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }
}
